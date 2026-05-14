use glyim_diag::GlyimDiagnostic;
use glyim_span::FileId;
use glyim_syntax::{GlyimLang, GreenNode, SyntaxKind, SyntaxNode};
use rowan::Language;

#[derive(Clone, Debug)]
pub struct ParseResult {
    pub green_node: GreenNode,
    pub diagnostics: Vec<GlyimDiagnostic>,
    pub root: SyntaxNode,
}

pub(crate) struct Parser<'a> {
    tokens: &'a [crate::lexer::Token],
    pos: usize,
    builder: rowan::GreenNodeBuilder<'a>,
    diagnostics: Vec<GlyimDiagnostic>,
    last_was_path: bool,
    suppress_struct_lit: bool,
    pending_gt_count: u32,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(tokens: &'a [crate::lexer::Token]) -> Self {
        Self {
            tokens,
            pos: 0,
            builder: rowan::GreenNodeBuilder::new(),
            diagnostics: Vec::new(),
            last_was_path: false,
            suppress_struct_lit: false,
            pending_gt_count: 0,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    fn current(&self) -> Option<&crate::lexer::Token> {
        self.tokens.get(self.pos)
    }

    fn current_kind(&self) -> SyntaxKind {
        if self.pending_gt_count > 0 {
            return SyntaxKind::Gt;
        }
        self.current().map_or(SyntaxKind::Error, |t| t.kind)
    }

    fn bump(&mut self) {
        if self.pending_gt_count > 0 {
            self.builder
                .token(GlyimLang::kind_to_raw(SyntaxKind::Gt), ">");
            self.pending_gt_count -= 1;
            return;
        }
        if let Some(token) = self.current() {
            let kind = GlyimLang::kind_to_raw(token.kind);
            let text = token.text.clone();
            self.builder.token(kind, text.as_str());
            self.pos += 1;
        }
    }

    fn expect(&mut self, expected: SyntaxKind) {
        if self.current_kind() == expected {
            self.bump();
        } else {
            self.error(format!(
                "expected {:?}, found {:?}",
                expected,
                self.current_kind()
            ));
        }
    }

    fn error(&mut self, message: impl Into<String>) {
        let span = self
            .current()
            .map(|t| t.span)
            .unwrap_or(glyim_span::Span::DUMMY);
        self.diagnostics
            .push(GlyimDiagnostic::parse_error(span, message));
    }

    fn bump_expected(&mut self, expected: SyntaxKind) {
        if self.current_kind() != expected {
            self.error(format!(
                "expected {:?}, found {:?}",
                expected,
                self.current_kind()
            ));
        }
        if self.current().is_some() || self.pending_gt_count > 0 {
            self.bump();
        }
    }

    fn checkpoint(&self) -> rowan::Checkpoint {
        self.builder.checkpoint()
    }

    fn start_node_at(&mut self, checkpoint: rowan::Checkpoint, kind: SyntaxKind) {
        self.builder
            .start_node_at(checkpoint, GlyimLang::kind_to_raw(kind));
    }

    fn peek_kind(&self) -> Option<SyntaxKind> {
        self.tokens.get(self.pos + 1).map(|t| t.kind)
    }

    fn skip_token(&mut self) {
        if self.current().is_some() {
            self.pos += 1;
        }
    }

    fn start_node(&mut self, kind: SyntaxKind) {
        let raw_kind = GlyimLang::kind_to_raw(kind);
        self.builder.start_node(raw_kind);
    }

    fn finish_node(&mut self) {
        self.builder.finish_node();
    }

    // ---- TOP LEVEL ----

    pub(crate) fn parse_source_file(&mut self) {
        self.start_node(SyntaxKind::SourceFile);
        while self.current().is_some() {
            self.parse_item();
        }
        self.finish_node();
    }

    fn parse_item(&mut self) {
        let _vis = self.parse_visibility();

        if self.current_kind() == SyntaxKind::KwUnsafe {
            self.bump(); // unsafe
            if !matches!(
                self.current_kind(),
                SyntaxKind::KwFn
                    | SyntaxKind::KwStruct
                    | SyntaxKind::KwEnum
                    | SyntaxKind::KwTrait
                    | SyntaxKind::KwImpl
                    | SyntaxKind::KwMod
                    | SyntaxKind::KwConst
                    | SyntaxKind::KwStatic
                    | SyntaxKind::KwType
                    | SyntaxKind::KwExtern
            ) {
                self.error("expected item after 'unsafe'");
                return;
            }
        }

        match self.current_kind() {
            SyntaxKind::KwFn => self.parse_fn_def(),
            SyntaxKind::KwStruct => self.parse_struct_def(),
            SyntaxKind::KwEnum => self.parse_enum_def(),
            SyntaxKind::KwTrait => self.parse_trait_def(),
            SyntaxKind::KwImpl => self.parse_impl_def(),
            SyntaxKind::KwMod => {
                self.start_node(SyntaxKind::Module);
                self.bump(); // mod
                self.bump_expected(SyntaxKind::Ident);
                if self.current_kind() == SyntaxKind::LBrace {
                    self.bump(); // {
                    while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
                        self.parse_item();
                    }
                    self.expect(SyntaxKind::RBrace);
                } else {
                    self.expect(SyntaxKind::Semicolon);
                }
                self.finish_node(); // Module
            }
            SyntaxKind::KwConst => {
                tracing::warn!("STUB: const parsing not yet implemented");
                self.bump(); // const
                self.bump_expected(SyntaxKind::Ident);
                self.expect(SyntaxKind::Colon);
                self.parse_type();
                if self.current_kind() == SyntaxKind::Eq {
                    self.bump();
                    self.parse_expr();
                }
                self.expect(SyntaxKind::Semicolon);
            }
            SyntaxKind::KwStatic => {
                tracing::warn!("STUB: static parsing not yet implemented");
                self.bump(); // static
                if self.current_kind() == SyntaxKind::KwRef {
                    self.bump();
                }
                if self.current_kind() == SyntaxKind::KwMut {
                    self.bump();
                }
                self.bump_expected(SyntaxKind::Ident);
                self.expect(SyntaxKind::Colon);
                self.parse_type();
                if self.current_kind() == SyntaxKind::Eq {
                    self.bump();
                    self.parse_expr();
                }
                self.expect(SyntaxKind::Semicolon);
            }
            SyntaxKind::KwType => {
                tracing::warn!("STUB: type alias parsing not yet implemented");
                self.bump(); // type
                self.bump_expected(SyntaxKind::Ident);
                if self.current_kind() == SyntaxKind::Lt {
                    self.parse_type_param_list();
                }
                if self.current_kind() == SyntaxKind::Eq {
                    self.bump();
                    self.parse_type();
                }
                self.expect(SyntaxKind::Semicolon);
            }
            SyntaxKind::KwExtern => {
                tracing::warn!("STUB: extern block parsing not yet implemented");
                self.bump(); // extern
                if self.current_kind() == SyntaxKind::StringLit {
                    self.bump(); // ABI string
                }
                if self.current_kind() == SyntaxKind::LBrace {
                    self.parse_block_inner();
                } else {
                    self.expect(SyntaxKind::Semicolon);
                }
            }
            _ => {
                self.error(format!("expected item, found {:?}", self.current_kind()));
                while self.current().is_some()
                    && !matches!(
                        self.current_kind(),
                        SyntaxKind::KwFn
                            | SyntaxKind::KwStruct
                            | SyntaxKind::KwEnum
                            | SyntaxKind::KwTrait
                            | SyntaxKind::KwImpl
                            | SyntaxKind::KwMod
                            | SyntaxKind::KwConst
                            | SyntaxKind::KwStatic
                            | SyntaxKind::KwType
                            | SyntaxKind::KwExtern
                            | SyntaxKind::KwPub
                            | SyntaxKind::KwUnsafe
                    )
                {
                    self.bump();
                }
            }
        }
    }

    fn parse_visibility(&mut self) -> bool {
        if self.current_kind() == SyntaxKind::KwPub {
            self.bump();
            true
        } else {
            false
        }
    }

    // ---- FUNCTION ----

    fn parse_fn_def(&mut self) {
        self.start_node(SyntaxKind::FnDef);
        self.bump_expected(SyntaxKind::KwFn);
        self.bump_expected(SyntaxKind::Ident);
        if self.current_kind() == SyntaxKind::Lt {
            self.parse_type_param_list();
        }
        self.expect(SyntaxKind::LParen);
        self.start_node(SyntaxKind::ParamList);
        while self.current_kind() != SyntaxKind::RParen && self.current().is_some() {
            self.parse_param();
            if self.current_kind() == SyntaxKind::Comma {
                self.bump();
            }
        }
        self.finish_node(); // ParamList
        self.expect(SyntaxKind::RParen);
        if self.current_kind() == SyntaxKind::Arrow {
            self.bump();
            self.parse_type();
        }
        if self.current_kind() == SyntaxKind::LBrace {
            self.parse_block();
        } else {
            self.expect(SyntaxKind::Semicolon);
        }
        self.finish_node(); // FnDef
    }

    fn parse_param(&mut self) {
        self.start_node(SyntaxKind::Param);
        if self.current_kind() == SyntaxKind::And {
            self.bump(); // &
            if self.current_kind() == SyntaxKind::KwMut {
                self.bump(); // mut
            }
            if self.current_kind() == SyntaxKind::KwSelf {
                self.bump(); // self
                self.finish_node();
                return;
            }
            self.bump_expected(SyntaxKind::Ident);
            self.expect(SyntaxKind::Colon);
            self.parse_type();
        } else if self.current_kind() == SyntaxKind::KwMut {
            self.bump(); // mut
            if self.current_kind() == SyntaxKind::KwSelf {
                self.bump(); // self
                self.finish_node();
                return;
            }
            self.bump_expected(SyntaxKind::Ident);
            self.expect(SyntaxKind::Colon);
            self.parse_type();
        } else if self.current_kind() == SyntaxKind::KwSelf {
            self.bump(); // self
            self.finish_node();
            return;
        } else {
            self.bump_expected(SyntaxKind::Ident);
            self.expect(SyntaxKind::Colon);
            self.parse_type();
        }
        self.finish_node(); // Param
    }

    // ---- STRUCT ----

    fn parse_struct_def(&mut self) {
        self.start_node(SyntaxKind::StructDef);
        self.bump_expected(SyntaxKind::KwStruct);
        self.bump_expected(SyntaxKind::Ident);
        if self.current_kind() == SyntaxKind::Lt {
            self.parse_type_param_list();
        }
        match self.current_kind() {
            SyntaxKind::LParen => {
                self.bump(); // (
                while self.current_kind() != SyntaxKind::RParen && self.current().is_some() {
                    self.parse_type();
                    if self.current_kind() == SyntaxKind::Comma {
                        self.bump();
                    }
                }
                self.expect(SyntaxKind::RParen);
                self.expect(SyntaxKind::Semicolon);
            }
            SyntaxKind::LBrace => {
                self.bump(); // {
                while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
                    if self.current_kind() == SyntaxKind::Ident {
                        self.bump();
                        if self.current_kind() == SyntaxKind::Colon {
                            self.bump();
                            self.parse_type();
                        }
                    } else {
                        self.error("expected field name");
                        self.bump();
                    }
                    if self.current_kind() == SyntaxKind::Comma {
                        self.bump();
                    }
                }
                self.expect(SyntaxKind::RBrace);
            }
            _ => {
                self.expect(SyntaxKind::Semicolon);
            }
        }
        self.finish_node(); // StructDef
    }

    // ---- ENUM ----

    fn parse_enum_def(&mut self) {
        self.start_node(SyntaxKind::EnumDef);
        self.bump_expected(SyntaxKind::KwEnum);
        self.bump_expected(SyntaxKind::Ident);
        if self.current_kind() == SyntaxKind::Lt {
            self.parse_type_param_list();
        }
        self.expect(SyntaxKind::LBrace);
        self.start_node(SyntaxKind::VariantList);
        while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
            self.start_node(SyntaxKind::EnumVariant);
            self.bump_expected(SyntaxKind::Ident);
            if self.current_kind() == SyntaxKind::LParen {
                self.bump(); // (
                while self.current_kind() != SyntaxKind::RParen && self.current().is_some() {
                    self.parse_type();
                    if self.current_kind() == SyntaxKind::Comma {
                        self.bump();
                    }
                }
                self.expect(SyntaxKind::RParen);
            }
            if self.current_kind() == SyntaxKind::LBrace {
                self.start_node(SyntaxKind::FieldList);
                self.bump(); // {
                while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
                    self.start_node(SyntaxKind::StructField);
                    if self.current_kind() == SyntaxKind::Ident {
                        self.bump(); // field name
                        if self.current_kind() == SyntaxKind::Colon {
                            self.bump(); // :
                            self.parse_type();
                        }
                    } else {
                        self.error("expected field name");
                        if self.current().is_some() {
                            self.bump();
                        }
                    }
                    self.finish_node(); // StructField
                    if self.current_kind() == SyntaxKind::Comma {
                        self.bump();
                    }
                }
                self.finish_node(); // FieldList
                self.expect(SyntaxKind::RBrace);
            }
            if self.current_kind() == SyntaxKind::Eq {
                self.bump();
                self.parse_expr();
            }
            self.finish_node(); // EnumVariant
            if self.current_kind() == SyntaxKind::Comma {
                self.bump();
            }
        }
        self.finish_node(); // VariantList
        self.expect(SyntaxKind::RBrace);
        self.finish_node(); // EnumDef
    }

    // ---- TRAIT ----

    fn parse_trait_def(&mut self) {
        self.start_node(SyntaxKind::TraitDef);
        self.bump_expected(SyntaxKind::KwTrait);
        self.bump_expected(SyntaxKind::Ident);
        if self.current_kind() == SyntaxKind::Lt {
            self.parse_type_param_list();
        }
        if self.current_kind() == SyntaxKind::Colon {
            self.bump(); // :
            loop {
                self.parse_type();
                if self.current_kind() == SyntaxKind::Plus {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        if self.current_kind() == SyntaxKind::KwWhere {
            self.parse_where_clause();
        }
        self.expect(SyntaxKind::LBrace);
        while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
            match self.current_kind() {
                SyntaxKind::KwFn => self.parse_fn_def(),
                SyntaxKind::KwType => {
                    tracing::warn!("STUB: associated type in trait");
                    self.bump(); // type
                    self.bump_expected(SyntaxKind::Ident);
                    if self.current_kind() == SyntaxKind::Colon {
                        self.bump();
                        loop {
                            self.parse_type();
                            if self.current_kind() == SyntaxKind::Plus {
                                self.bump();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(SyntaxKind::Semicolon);
                }
                SyntaxKind::KwConst => {
                    tracing::warn!("STUB: associated const in trait");
                    self.bump(); // const
                    self.bump_expected(SyntaxKind::Ident);
                    self.expect(SyntaxKind::Colon);
                    self.parse_type();
                    if self.current_kind() == SyntaxKind::Eq {
                        self.bump();
                        self.parse_expr();
                    }
                    self.expect(SyntaxKind::Semicolon);
                }
                _ => {
                    self.error(format!(
                        "expected trait item, found {:?}",
                        self.current_kind()
                    ));
                    self.bump();
                }
            }
        }
        self.expect(SyntaxKind::RBrace);
        self.finish_node(); // TraitDef
    }

    // ---- IMPL ----

    fn parse_impl_def(&mut self) {
        self.start_node(SyntaxKind::ImplDef);
        self.bump_expected(SyntaxKind::KwImpl);
        if self.current_kind() == SyntaxKind::Lt {
            self.parse_type_param_list();
        }
        self.parse_type();
        if self.current_kind() == SyntaxKind::KwFor {
            self.bump(); // for
            self.parse_type();
        }
        if self.current_kind() == SyntaxKind::KwWhere {
            self.parse_where_clause();
        }
        self.expect(SyntaxKind::LBrace);
        while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
            match self.current_kind() {
                SyntaxKind::KwFn => self.parse_fn_def(),
                SyntaxKind::KwType => {
                    tracing::warn!("STUB: associated type in impl");
                    self.bump(); // type
                    self.bump_expected(SyntaxKind::Ident);
                    if self.current_kind() == SyntaxKind::Eq {
                        self.bump();
                        self.parse_type();
                    }
                    self.expect(SyntaxKind::Semicolon);
                }
                SyntaxKind::KwConst => {
                    tracing::warn!("STUB: associated const in impl");
                    self.bump(); // const
                    self.bump_expected(SyntaxKind::Ident);
                    self.expect(SyntaxKind::Colon);
                    self.parse_type();
                    self.expect(SyntaxKind::Eq);
                    self.parse_expr();
                    self.expect(SyntaxKind::Semicolon);
                }
                _ => {
                    self.error(format!(
                        "expected impl item, found {:?}",
                        self.current_kind()
                    ));
                    self.bump();
                }
            }
        }
        self.expect(SyntaxKind::RBrace);
        self.finish_node(); // ImplDef
    }

    fn parse_where_clause(&mut self) {
        self.start_node(SyntaxKind::WhereClause);
        self.expect(SyntaxKind::KwWhere);
        while self.current_kind() != SyntaxKind::LBrace
            && self.current_kind() != SyntaxKind::Semicolon
            && self.current().is_some()
        {
            self.parse_type();
            if self.current_kind() == SyntaxKind::Colon {
                self.bump(); // :
                loop {
                    self.parse_type();
                    if self.current_kind() == SyntaxKind::Lt {
                        self.parse_type_param_list();
                    }
                    if self.current_kind() == SyntaxKind::Plus {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            if self.current_kind() == SyntaxKind::Comma {
                self.bump();
            } else if self.current_kind() != SyntaxKind::LBrace
                && self.current_kind() != SyntaxKind::Semicolon
            {
                self.error("expected ',' or end of where clause");
                break;
            }
        }
        self.finish_node();
    }

    // ---- TYPE PARAMS ----

    fn parse_type_param_list(&mut self) {
        self.start_node(SyntaxKind::TypeParamList);
        self.expect(SyntaxKind::Lt);
        while self.current_kind() != SyntaxKind::Gt
            && self.current_kind() != SyntaxKind::Shr
            && self.current().is_some()
        {
            self.start_node(SyntaxKind::TypeParam);
            self.bump_expected(SyntaxKind::Ident);
            if self.current_kind() == SyntaxKind::Colon {
                self.bump();
                loop {
                    self.parse_type();
                    if self.current_kind() == SyntaxKind::Plus {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            if self.current_kind() == SyntaxKind::Eq {
                self.bump();
                self.parse_type();
            }
            self.finish_node(); // TypeParam
            if self.current_kind() == SyntaxKind::Comma {
                self.bump();
            }
        }
        // Handle >> as two Gt tokens: emit both, one for this list, one queued
        if self.current_kind() == SyntaxKind::Shr {
            self.builder
                .token(GlyimLang::kind_to_raw(SyntaxKind::Gt), ">");
            self.builder
                .token(GlyimLang::kind_to_raw(SyntaxKind::Gt), ">");
            self.pos += 1; // skip the Shr token
            self.pending_gt_count += 1;
            // Do NOT consume from pending here.
            self.finish_node();
            return;
        }
        // Not Shr - consume a real Gt token
        if self.current_kind() == SyntaxKind::Gt {
            self.bump();
        }
        self.finish_node();
    }

    fn parse_type_arg_list(&mut self) {
        self.bump(); // <
        while self.current_kind() != SyntaxKind::Gt
            && self.current_kind() != SyntaxKind::Shr
            && self.current().is_some()
        {
            self.parse_type();
            if self.current_kind() == SyntaxKind::Comma {
                self.bump();
            }
        }
        // Handle >> as two Gt tokens:
        // Emit both now. The first closes this list, the second is queued for the outer list.
        if self.current_kind() == SyntaxKind::Shr {
            // Emit Gt for THIS list (closes this type arg list)
            self.builder
                .token(GlyimLang::kind_to_raw(SyntaxKind::Gt), ">");
            // Emit Gt for the OUTER list (queued via pending_gt_count)
            self.builder
                .token(GlyimLang::kind_to_raw(SyntaxKind::Gt), ">");
            self.pos += 1; // skip the Shr token
            self.pending_gt_count += 1; // outer list will see Gt via current_kind
            // Do NOT consume from pending here - the Gt for this list was already emitted.
            return;
        }
        // Not Shr - consume a real Gt token
        if self.current_kind() == SyntaxKind::Gt {
            self.bump();
        }
    }

    // ---- BLOCKS & STMTS ----

    fn parse_block(&mut self) {
        self.start_node(SyntaxKind::Block);
        self.expect(SyntaxKind::LBrace);
        self.parse_block_inner();
        self.expect(SyntaxKind::RBrace);
        self.finish_node();
    }

    fn parse_block_inner(&mut self) {
        while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
            self.parse_stmt();
        }
    }

    fn parse_stmt(&mut self) {
        match self.current_kind() {
            SyntaxKind::KwLet => {
                self.start_node(SyntaxKind::LetStmt);
                self.bump(); // let
                self.parse_pat();
                if self.current_kind() == SyntaxKind::Colon {
                    self.bump();
                    self.parse_type();
                }
                if self.current_kind() == SyntaxKind::Eq {
                    self.bump();
                    self.parse_expr();
                }
                if self.current_kind() == SyntaxKind::Semicolon {
                    self.bump();
                } else {
                    self.error("expected ';' after let statement");
                }
                self.finish_node();
            }
            SyntaxKind::KwIf
            | SyntaxKind::KwWhile
            | SyntaxKind::KwFor
            | SyntaxKind::KwLoop
            | SyntaxKind::KwReturn
            | SyntaxKind::KwBreak
            | SyntaxKind::KwContinue
            | SyntaxKind::KwMove => {
                self.start_node(SyntaxKind::ExprStmt);
                self.parse_expr();
                if self.current_kind() == SyntaxKind::Semicolon {
                    self.bump();
                }
                self.finish_node();
            }
            SyntaxKind::KwFn
            | SyntaxKind::KwStruct
            | SyntaxKind::KwEnum
            | SyntaxKind::KwTrait
            | SyntaxKind::KwImpl
            | SyntaxKind::KwMod
            | SyntaxKind::KwConst
            | SyntaxKind::KwStatic
            | SyntaxKind::KwType
            | SyntaxKind::KwExtern
            | SyntaxKind::KwPub => self.parse_item(),
            SyntaxKind::KwUnsafe => {
                // Look ahead: if next is LBrace, it's an unsafe block expression
                if self.peek_kind() == Some(SyntaxKind::LBrace) {
                    self.start_node(SyntaxKind::ExprStmt);
                    self.parse_expr();
                    self.finish_node();
                } else {
                    self.parse_item();
                }
            }
            SyntaxKind::LBrace => {
                self.start_node(SyntaxKind::ExprStmt);
                self.parse_expr();
                self.finish_node();
            }
            _ => {
                if !matches!(
                    self.current_kind(),
                    SyntaxKind::Ident
                        | SyntaxKind::IntLit
                        | SyntaxKind::FloatLit
                        | SyntaxKind::StringLit
                        | SyntaxKind::CharLit
                        | SyntaxKind::KwTrue
                        | SyntaxKind::KwFalse
                        | SyntaxKind::KwSelf
                        | SyntaxKind::KwSuper
                        | SyntaxKind::KwCrate
                        | SyntaxKind::LParen
                        | SyntaxKind::LBrace
                        | SyntaxKind::LBracket
                        | SyntaxKind::Or
                        | SyntaxKind::OrOr
                        | SyntaxKind::KwIf
                        | SyntaxKind::KwWhile
                        | SyntaxKind::KwFor
                        | SyntaxKind::KwLoop
                        | SyntaxKind::KwMatch
                        | SyntaxKind::KwReturn
                        | SyntaxKind::KwBreak
                        | SyntaxKind::KwContinue
                        | SyntaxKind::Bang
                        | SyntaxKind::Minus
                        | SyntaxKind::Star
                        | SyntaxKind::And
                        | SyntaxKind::KwUnsafe
                ) {
                    self.error(format!(
                        "unexpected token in statement: {:?}",
                        self.current_kind()
                    ));
                    if self.current().is_some() {
                        self.bump();
                    }
                    return;
                }
                self.start_node(SyntaxKind::ExprStmt);
                self.parse_expr();
                if self.current_kind() == SyntaxKind::Semicolon {
                    self.bump();
                } else if self.current_kind() == SyntaxKind::RBrace {
                    // tail expression
                } else {
                    self.error("expected ';' after expression");
                }
                self.finish_node();
            }
        }
    }

    // ---- EXPRESSIONS ----

    fn parse_expr(&mut self) {
        self.parse_assignment_expr();
    }

    fn parse_assignment_expr(&mut self) {
        let cp = self.checkpoint();
        self.parse_range_expr();
        if matches!(
            self.current_kind(),
            SyntaxKind::Eq
                | SyntaxKind::PlusEq
                | SyntaxKind::MinusEq
                | SyntaxKind::StarEq
                | SyntaxKind::SlashEq
        ) {
            self.start_node_at(cp, SyntaxKind::AssignExpr);
            self.bump();
            self.parse_assignment_expr();
            self.finish_node();
        }
    }

    fn parse_range_expr(&mut self) {
        let cp = self.checkpoint();
        self.parse_or_expr();
        if matches!(
            self.current_kind(),
            SyntaxKind::DotDot | SyntaxKind::DotDotEq
        ) {
            self.start_node_at(cp, SyntaxKind::RangeExpr);
            self.bump();
            if !matches!(
                self.current_kind(),
                SyntaxKind::Eq
                    | SyntaxKind::Semicolon
                    | SyntaxKind::Comma
                    | SyntaxKind::RParen
                    | SyntaxKind::RBrace
                    | SyntaxKind::RBracket
            ) {
                self.parse_or_expr();
            }
            self.finish_node();
        }
    }

    fn parse_or_expr(&mut self) {
        self.parse_and_expr();
        while self.current_kind() == SyntaxKind::OrOr {
            self.start_node(SyntaxKind::BinaryExpr);
            self.bump();
            self.parse_and_expr();
            self.finish_node();
        }
    }

    fn parse_and_expr(&mut self) {
        self.parse_comparison_expr();
        while self.current_kind() == SyntaxKind::AndAnd {
            self.start_node(SyntaxKind::BinaryExpr);
            self.bump();
            self.parse_comparison_expr();
            self.finish_node();
        }
    }

    fn parse_comparison_expr(&mut self) {
        let cp = self.checkpoint();
        self.parse_bitwise_expr();
        if matches!(
            self.current_kind(),
            SyntaxKind::EqEq
                | SyntaxKind::BangEq
                | SyntaxKind::Lt
                | SyntaxKind::Gt
                | SyntaxKind::LtEq
                | SyntaxKind::GtEq
        ) {
            self.start_node_at(cp, SyntaxKind::BinaryExpr);
            self.bump();
            self.parse_additive_expr();
            self.finish_node();
        }
    }

    fn parse_bitwise_expr(&mut self) {
        let mut cp = self.checkpoint();
        self.parse_additive_expr();
        while matches!(
            self.current_kind(),
            SyntaxKind::And
                | SyntaxKind::Or
                | SyntaxKind::Caret
                | SyntaxKind::Shl
                | SyntaxKind::Shr
        ) {
            self.start_node_at(cp, SyntaxKind::BinaryExpr);
            self.bump();
            self.parse_additive_expr();
            self.finish_node();
            cp = self.checkpoint();
        }
    }

    fn parse_additive_expr(&mut self) {
        let mut cp = self.checkpoint();
        self.parse_multiplicative_expr();
        while matches!(self.current_kind(), SyntaxKind::Plus | SyntaxKind::Minus) {
            self.start_node_at(cp, SyntaxKind::BinaryExpr);
            self.bump();
            self.parse_multiplicative_expr();
            self.finish_node();
            cp = self.checkpoint();
        }
    }

    fn parse_multiplicative_expr(&mut self) {
        let mut cp = self.checkpoint();
        self.parse_unary_expr();
        while matches!(
            self.current_kind(),
            SyntaxKind::Star | SyntaxKind::Slash | SyntaxKind::Percent
        ) {
            self.start_node_at(cp, SyntaxKind::BinaryExpr);
            self.bump();
            self.parse_unary_expr();
            self.finish_node();
            cp = self.checkpoint();
        }
    }

    fn parse_unary_expr(&mut self) {
        if matches!(
            self.current_kind(),
            SyntaxKind::Bang | SyntaxKind::Minus | SyntaxKind::Star | SyntaxKind::And
        ) {
            self.start_node(SyntaxKind::UnaryExpr);
            self.bump();
            self.parse_unary_expr();
            self.finish_node();
        } else {
            self.parse_postfix_expr();
        }
    }

    fn parse_postfix_expr(&mut self) {
        let cp = self.checkpoint();
        self.parse_primary_expr();
        loop {
            match self.current_kind() {
                SyntaxKind::Dot => {
                    self.bump();
                    if matches!(
                        self.current_kind(),
                        SyntaxKind::Ident | SyntaxKind::IntLit | SyntaxKind::FloatLit
                    ) {
                        self.bump();
                    } else {
                        self.error("expected field name or index after '.'");
                    }
                    if self.current_kind() == SyntaxKind::ColonColon {
                        self.bump();
                        if self.current_kind() == SyntaxKind::Lt {
                            self.parse_type_arg_list();
                        }
                    }
                    if self.current_kind() == SyntaxKind::LParen {
                        self.start_node_at(cp, SyntaxKind::MethodCallExpr);
                        self.bump();
                        if self.current_kind() != SyntaxKind::RParen {
                            self.parse_expr();
                            while self.current_kind() == SyntaxKind::Comma {
                                self.bump();
                                if self.current_kind() == SyntaxKind::RParen {
                                    break;
                                }
                                self.parse_expr();
                            }
                        }
                        self.expect(SyntaxKind::RParen);
                        self.finish_node();
                    } else {
                        self.start_node_at(cp, SyntaxKind::FieldExpr);
                        self.finish_node();
                    }
                }
                SyntaxKind::LParen => {
                    self.start_node_at(cp, SyntaxKind::CallExpr);
                    self.bump();
                    if self.current_kind() != SyntaxKind::RParen {
                        self.parse_expr();
                        while self.current_kind() == SyntaxKind::Comma {
                            self.bump();
                            if self.current_kind() == SyntaxKind::RParen {
                                break;
                            }
                            self.parse_expr();
                        }
                    }
                    self.expect(SyntaxKind::RParen);
                    self.finish_node();
                }
                SyntaxKind::LBracket => {
                    self.start_node_at(cp, SyntaxKind::IndexExpr);
                    self.bump();
                    self.parse_expr();
                    self.expect(SyntaxKind::RBracket);
                    self.finish_node();
                }
                SyntaxKind::Question => {
                    self.bump();
                }
                SyntaxKind::KwAs => {
                    self.start_node_at(cp, SyntaxKind::CastExpr);
                    self.bump();
                    self.parse_type();
                    self.finish_node();
                }
                SyntaxKind::LBrace if self.last_was_path && !self.suppress_struct_lit => {
                    self.start_node_at(cp, SyntaxKind::StructExpr);
                    self.bump();
                    while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
                        if self.current_kind() == SyntaxKind::Ident {
                            self.bump();
                            if self.current_kind() == SyntaxKind::Colon {
                                self.bump();
                                self.parse_expr();
                            }
                        } else if self.current_kind() == SyntaxKind::DotDot {
                            self.bump();
                            if self.current_kind() != SyntaxKind::RBrace {
                                self.parse_expr();
                            }
                        } else {
                            self.error("expected field name or '..'");
                            if self.current().is_some() {
                                self.bump();
                            }
                        }
                        if self.current_kind() == SyntaxKind::Comma {
                            self.bump();
                        }
                    }
                    self.expect(SyntaxKind::RBrace);
                    self.finish_node();
                }
                _ => break,
            }
        }
    }

    fn parse_primary_expr(&mut self) {
        self.last_was_path = false;
        match self.current_kind() {
            SyntaxKind::Ident | SyntaxKind::KwSelf | SyntaxKind::KwSuper | SyntaxKind::KwCrate => {
                self.parse_path_expr();
            }
            SyntaxKind::IntLit
            | SyntaxKind::FloatLit
            | SyntaxKind::StringLit
            | SyntaxKind::CharLit
            | SyntaxKind::KwTrue
            | SyntaxKind::KwFalse => {
                self.start_node(SyntaxKind::LitExpr);
                self.bump();
                self.finish_node();
            }
            SyntaxKind::LParen => {
                let cp = self.checkpoint();
                self.bump(); // (
                if self.current_kind() == SyntaxKind::RParen {
                    self.bump(); // )
                    // empty tuple
                    self.start_node_at(cp, SyntaxKind::TupleExpr);
                    self.finish_node();
                } else {
                    self.parse_expr();
                    if self.current_kind() == SyntaxKind::Comma {
                        // Multiple elements: wrap in TupleExpr
                        self.start_node_at(cp, SyntaxKind::TupleExpr);
                        while self.current_kind() == SyntaxKind::Comma {
                            self.bump();
                            self.parse_expr();
                        }
                        self.expect(SyntaxKind::RParen);
                        self.finish_node(); // TupleExpr
                    } else {
                        // Single expression: just parenthesized, not a tuple
                        self.expect(SyntaxKind::RParen);
                    }
                }
            }
            SyntaxKind::KwMove => {
                self.parse_closure_expr();
            }
            SyntaxKind::Or => self.parse_closure_expr(),
            SyntaxKind::KwUnsafe => {
                self.bump(); // unsafe
                if self.current_kind() == SyntaxKind::LBrace {
                    self.parse_block();
                } else {
                    self.error("expected '{' after unsafe");
                }
            }
            SyntaxKind::LBrace => self.parse_block(),
            SyntaxKind::KwIf => self.parse_if_expr(),
            SyntaxKind::KwWhile => self.parse_while_expr(),
            SyntaxKind::KwLoop => self.parse_loop_expr(),
            SyntaxKind::KwFor => self.parse_for_expr(),
            SyntaxKind::KwReturn => {
                self.start_node(SyntaxKind::ReturnExpr);
                self.bump(); // return
                if !matches!(
                    self.current_kind(),
                    SyntaxKind::Semicolon | SyntaxKind::RBrace
                ) {
                    self.parse_expr();
                }
                self.finish_node(); // ReturnExpr
            }
            SyntaxKind::KwBreak => {
                self.start_node(SyntaxKind::BreakExpr);
                self.bump(); // break
                if !matches!(
                    self.current_kind(),
                    SyntaxKind::Semicolon | SyntaxKind::RBrace
                ) {
                    self.parse_expr();
                }
                self.finish_node(); // BreakExpr
            }
            SyntaxKind::KwContinue => {
                self.start_node(SyntaxKind::ContinueExpr);
                self.bump(); // continue
                self.finish_node(); // ContinueExpr
            }
            SyntaxKind::LBracket => {
                self.start_node(SyntaxKind::ArrayExpr);
                self.bump(); // [
                if self.current_kind() == SyntaxKind::RBracket {
                    // empty array
                } else {
                    self.parse_expr();
                    if self.current_kind() == SyntaxKind::Semicolon {
                        self.bump(); // ;
                        self.parse_expr(); // count
                    } else {
                        while self.current_kind() == SyntaxKind::Comma {
                            self.bump();
                            if self.current_kind() == SyntaxKind::RBracket {
                                break;
                            }
                            self.parse_expr();
                        }
                    }
                }
                self.expect(SyntaxKind::RBracket);
                self.finish_node();
            }
            SyntaxKind::KwMatch => {
                self.start_node(SyntaxKind::MatchExpr);
                self.bump(); // match
                self.suppress_struct_lit = true;
                self.parse_expr();
                self.suppress_struct_lit = false;
                self.expect(SyntaxKind::LBrace);
                self.start_node(SyntaxKind::MatchArmList);
                while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
                    self.parse_match_arm();
                }
                self.finish_node(); // MatchArmList
                self.expect(SyntaxKind::RBrace);
                self.finish_node(); // MatchExpr
            }
            _ => {
                self.error(format!(
                    "expected expression, found {:?}",
                    self.current_kind()
                ));
                if self.current().is_some() {
                    self.bump();
                }
            }
        }
    }

    fn parse_path_expr(&mut self) {
        self.start_node(SyntaxKind::PathExpr);
        self.parse_path();
        self.finish_node();
        self.last_was_path = true;
    }

    fn parse_path(&mut self) {
        self.start_node(SyntaxKind::UsePath);
        self.parse_path_inner();
        self.finish_node();
    }

    fn parse_path_inner(&mut self) {
        // First segment
        match self.current_kind() {
            SyntaxKind::Ident | SyntaxKind::KwSelf | SyntaxKind::KwSuper | SyntaxKind::KwCrate => {
                self.bump();
            }
            _ => {
                self.error("expected identifier in path");
                return;
            }
        }
        while self.current_kind() == SyntaxKind::ColonColon {
            self.bump(); // ::
            match self.current_kind() {
                SyntaxKind::Ident | SyntaxKind::KwSelf | SyntaxKind::KwSuper => {
                    self.bump();
                }
                SyntaxKind::Lt => {
                    self.parse_type_arg_list();
                }
                _ => {
                    self.error("expected identifier after '::'");
                    break;
                }
            }
        }
    }

    fn parse_if_expr(&mut self) {
        self.start_node(SyntaxKind::IfExpr);
        self.bump_expected(SyntaxKind::KwIf);
        self.suppress_struct_lit = true;
        if self.current_kind() == SyntaxKind::KwLet {
            self.bump(); // let
            self.parse_pat();
            self.expect(SyntaxKind::Eq);
            self.parse_expr();
        } else {
            self.parse_expr();
        }
        self.suppress_struct_lit = false;
        self.parse_block();
        if self.current_kind() == SyntaxKind::KwElse {
            self.bump();
            if self.current_kind() == SyntaxKind::KwIf {
                self.parse_if_expr();
            } else {
                self.parse_block();
            }
        }
        self.finish_node();
    }

    fn parse_while_expr(&mut self) {
        self.start_node(SyntaxKind::WhileExpr);
        self.bump(); // while
        self.suppress_struct_lit = true;
        if self.current_kind() == SyntaxKind::KwLet {
            self.bump(); // let
            self.parse_pat();
            self.expect(SyntaxKind::Eq);
            self.parse_expr();
        } else {
            self.parse_expr();
        }
        self.suppress_struct_lit = false;
        self.parse_block();
        self.finish_node();
    }

    #[allow(dead_code)]
    fn parse_labeled_expr(&mut self) {
        tracing::warn!("STUB: labeled expression parsing");
        self.bump();
        if self.current_kind() == SyntaxKind::Colon {
            self.bump();
        }
        self.parse_expr();
    }

    fn parse_loop_expr(&mut self) {
        self.start_node(SyntaxKind::LoopExpr);
        self.bump(); // loop
        if self.current_kind() == SyntaxKind::LBrace {
            self.parse_block();
        } else {
            self.error("expected '{' after loop");
        }
        self.finish_node();
    }

    fn parse_for_expr(&mut self) {
        self.start_node(SyntaxKind::ForExpr);
        self.bump(); // for
        self.suppress_struct_lit = true;
        self.parse_pat();
        self.expect(SyntaxKind::KwIn);
        self.parse_expr();
        self.suppress_struct_lit = false;
        self.parse_block();
        self.finish_node();
    }

    fn parse_closure_expr(&mut self) {
        self.start_node(SyntaxKind::ClosureExpr);
        if self.current_kind() == SyntaxKind::KwMove {
            self.bump(); // move
        }
        if self.current_kind() == SyntaxKind::Or {
            self.bump(); // first |
            if self.current_kind() == SyntaxKind::Or {
                self.bump(); // second |
            } else {
                while self.current_kind() != SyntaxKind::Or && self.current().is_some() {
                    self.parse_pat_single();
                    if self.current_kind() == SyntaxKind::Colon {
                        self.bump();
                        self.parse_type();
                    }
                    if self.current_kind() == SyntaxKind::Comma {
                        self.bump();
                    }
                }
                self.expect(SyntaxKind::Or);
            }
        }
        if self.current_kind() == SyntaxKind::Arrow {
            self.bump();
            self.parse_type();
        }
        if self.current_kind() == SyntaxKind::LBrace {
            self.parse_block();
        } else {
            self.parse_expr();
        }
        self.finish_node();
    }

    fn parse_match_arm(&mut self) {
        self.start_node(SyntaxKind::MatchArm);
        self.parse_pat();
        if self.current_kind() == SyntaxKind::KwIf {
            self.bump();
            self.parse_expr();
        }
        self.expect(SyntaxKind::FatArrow);
        if self.current_kind() == SyntaxKind::LBrace {
            self.parse_block();
            if self.current_kind() == SyntaxKind::Comma {
                self.bump();
            }
        } else {
            self.parse_expr();
            if self.current_kind() == SyntaxKind::Comma {
                self.bump();
            }
        }
        self.finish_node();
    }

    // ---- PATTERNS ----

    fn parse_pat(&mut self) {
        let cp = self.checkpoint();
        self.parse_pat_single();
        if self.current_kind() == SyntaxKind::Or {
            self.start_node_at(cp, SyntaxKind::PatOr);
            while self.current_kind() == SyntaxKind::Or {
                self.bump(); // |
                self.parse_pat_single();
            }
            self.finish_node(); // PatOr
        }
    }

    fn parse_pat_single(&mut self) {
        match self.current_kind() {
            SyntaxKind::KwRef => {
                self.bump(); // ref
                if self.current_kind() == SyntaxKind::KwMut {
                    self.bump(); // mut
                }
                self.parse_pat_inner();
            }
            SyntaxKind::KwMut => {
                self.bump(); // mut
                self.parse_pat_inner();
            }
            SyntaxKind::AndAnd => {
                self.skip_token(); // skip &&
                self.parse_pat_inner();
            }
            SyntaxKind::And => {
                self.bump(); // &
                if self.current_kind() == SyntaxKind::KwMut {
                    self.bump(); // mut
                }
                self.parse_pat_inner();
            }
            SyntaxKind::LParen => {
                self.start_node(SyntaxKind::PatTuple);
                self.bump(); // (
                while self.current_kind() != SyntaxKind::RParen && self.current().is_some() {
                    self.parse_pat();
                    if self.current_kind() == SyntaxKind::Comma {
                        self.bump();
                    }
                }
                self.expect(SyntaxKind::RParen);
                self.finish_node();
            }
            _ => {
                self.parse_pat_inner();
            }
        }
    }

    fn parse_pat_inner(&mut self) {
        match self.current_kind() {
            SyntaxKind::Bang => {
                self.start_node(SyntaxKind::NeverType);
                self.bump(); // !
                self.finish_node();
            }
            SyntaxKind::Underscore => {
                self.start_node(SyntaxKind::PatWild);
                self.bump();
                self.finish_node();
            }
            SyntaxKind::Ident | SyntaxKind::KwSelf | SyntaxKind::KwSuper | SyntaxKind::KwCrate => {
                let next = self.peek_kind().unwrap_or(SyntaxKind::Error);
                if next == SyntaxKind::ColonColon
                    || next == SyntaxKind::LParen
                    || next == SyntaxKind::LBrace
                {
                    self.start_node(SyntaxKind::UsePath);
                    self.parse_path_inner();
                    self.finish_node();
                } else {
                    self.start_node(SyntaxKind::PatIdent);
                    self.bump();
                    self.finish_node();
                    return;
                }
                if self.current_kind() == SyntaxKind::LParen {
                    self.start_node(SyntaxKind::PatTuple);
                    self.bump(); // (
                    while self.current_kind() != SyntaxKind::RParen && self.current().is_some() {
                        self.parse_pat();
                        if self.current_kind() == SyntaxKind::Comma {
                            self.bump();
                        }
                    }
                    self.expect(SyntaxKind::RParen);
                    self.finish_node();
                } else if self.current_kind() == SyntaxKind::LBrace {
                    self.start_node(SyntaxKind::PatStruct);
                    self.bump(); // {
                    while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
                        if self.current_kind() == SyntaxKind::DotDot {
                            self.bump(); // ..
                            // trailing comma allowed after ..
                            if self.current_kind() == SyntaxKind::Comma {
                                self.bump();
                            }
                        } else if self.current_kind() == SyntaxKind::Ident {
                            let cp = self.checkpoint();
                            self.bump(); // field name
                            if self.current_kind() == SyntaxKind::Colon {
                                // explicit: field_name: pattern
                                self.start_node_at(cp, SyntaxKind::PatIdent);
                                self.finish_node();
                                self.bump(); // :
                                self.parse_pat();
                            } else {
                                // shorthand: field_name (binding)
                                self.start_node_at(cp, SyntaxKind::PatIdent);
                                self.finish_node();
                            }
                        } else {
                            self.error("expected field pattern");
                            if self.current().is_some() {
                                self.bump();
                            }
                        }
                        if self.current_kind() == SyntaxKind::Comma {
                            self.bump();
                        }
                    }
                    self.expect(SyntaxKind::RBrace);
                    self.finish_node();
                }
            }
            SyntaxKind::IntLit
            | SyntaxKind::FloatLit
            | SyntaxKind::StringLit
            | SyntaxKind::CharLit
            | SyntaxKind::KwTrue
            | SyntaxKind::KwFalse => {
                self.start_node(SyntaxKind::PatLit);
                self.bump();
                // Check for range pattern: 0..10
                if matches!(
                    self.current_kind(),
                    SyntaxKind::DotDot | SyntaxKind::DotDotEq
                ) {
                    self.bump(); // .. or ..=
                    if !matches!(
                        self.current_kind(),
                        SyntaxKind::FatArrow | SyntaxKind::Comma | SyntaxKind::RBrace
                    ) {
                        self.parse_pat();
                    }
                }
                self.finish_node();
            }
            _ => {
                self.error(format!("expected pattern, found {:?}", self.current_kind()));
                if self.current().is_some() {
                    self.bump();
                }
            }
        }
    }

    // ---- TYPES ----

    fn parse_type(&mut self) {
        match self.current_kind() {
            SyntaxKind::AndAnd => {
                if let Some(_tok) = self.current() {
                    self.start_node(SyntaxKind::RefType);
                    self.builder
                        .token(GlyimLang::kind_to_raw(SyntaxKind::And), "&");
                    self.start_node(SyntaxKind::RefType);
                    self.builder
                        .token(GlyimLang::kind_to_raw(SyntaxKind::And), "&");
                    self.skip_token();
                    self.parse_type();
                    self.finish_node(); // inner
                    self.finish_node(); // outer
                }
            }
            SyntaxKind::And => {
                self.start_node(SyntaxKind::RefType);
                self.bump(); // &
                if self.current_kind() == SyntaxKind::KwMut {
                    self.bump(); // mut
                }
                self.parse_type();
                self.finish_node();
            }
            SyntaxKind::Star => {
                tracing::warn!("STUB: raw pointer type not supported");
                self.bump(); // *
                if self.current_kind() == SyntaxKind::KwConst
                    || self.current_kind() == SyntaxKind::KwMut
                {
                    self.bump();
                }
                self.parse_type();
            }
            SyntaxKind::LBracket => {
                self.start_node(SyntaxKind::ArrayType);
                self.bump(); // [
                self.parse_type();
                if self.current_kind() == SyntaxKind::Semicolon {
                    self.bump(); // ;
                    self.parse_expr();
                }
                self.expect(SyntaxKind::RBracket);
                self.finish_node();
            }
            SyntaxKind::LParen => {
                self.start_node(SyntaxKind::TupleType);
                self.bump(); // (
                while self.current_kind() != SyntaxKind::RParen && self.current().is_some() {
                    self.parse_type();
                    if self.current_kind() == SyntaxKind::Comma {
                        self.bump();
                    }
                }
                self.expect(SyntaxKind::RParen);
                self.finish_node();
            }
            SyntaxKind::Bang => {
                self.start_node(SyntaxKind::NeverType);
                self.bump(); // !
                self.finish_node();
            }
            SyntaxKind::Underscore => {
                self.start_node(SyntaxKind::InferType);
                self.bump();
                self.finish_node();
            }
            SyntaxKind::KwDyn => {
                self.start_node(SyntaxKind::DynType);
                self.bump(); // dyn
                loop {
                    self.parse_type();
                    if self.current_kind() == SyntaxKind::Plus {
                        self.bump();
                    } else {
                        break;
                    }
                }
                self.finish_node();
            }
            SyntaxKind::KwImpl => {
                tracing::warn!("STUB: impl trait type not supported");
                self.bump(); // impl
                loop {
                    self.parse_type();
                    if self.current_kind() == SyntaxKind::Plus {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            SyntaxKind::KwFn => {
                tracing::warn!("STUB: function pointer type not supported");
                self.bump(); // fn
                self.expect(SyntaxKind::LParen);
                while self.current_kind() != SyntaxKind::RParen && self.current().is_some() {
                    self.parse_type();
                    if self.current_kind() == SyntaxKind::Comma {
                        self.bump();
                    }
                }
                self.expect(SyntaxKind::RParen);
                if self.current_kind() == SyntaxKind::Arrow {
                    self.bump();
                    self.parse_type();
                }
            }
            SyntaxKind::Ident | SyntaxKind::KwSelf | SyntaxKind::KwSuper | SyntaxKind::KwCrate => {
                self.start_node(SyntaxKind::PathType);
                self.parse_path();
                if self.current_kind() == SyntaxKind::Lt {
                    self.parse_type_arg_list();
                }
                self.finish_node();
            }
            _ => {
                self.error(format!("expected type, found {:?}", self.current_kind()));
                if self.current().is_some() {
                    self.bump();
                }
            }
        }
    }

    // ---- FINISH ----

    fn finish(self) -> (GreenNode, Vec<GlyimDiagnostic>) {
        (self.builder.finish(), self.diagnostics)
    }
}

pub fn parse_to_syntax(source: &str, file_id: FileId) -> ParseResult {
    let lex_result = crate::lexer::lex(source, file_id);
    let mut parser = Parser::new(&lex_result.tokens);
    parser.parse_source_file();
    let (green_node, diagnostics) = parser.finish();
    let root = SyntaxNode::new_root(green_node.clone());
    let mut all_diagnostics = lex_result.diagnostics;
    all_diagnostics.extend(diagnostics);
    ParseResult {
        green_node,
        diagnostics: all_diagnostics,
        root,
    }
}
