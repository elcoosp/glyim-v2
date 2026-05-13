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
}

impl<'a> Parser<'a> {
    pub(crate) fn new(tokens: &'a [crate::lexer::Token]) -> Self {
        Self {
            tokens,
            pos: 0,
            builder: rowan::GreenNodeBuilder::new(),
            diagnostics: Vec::new(),
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
        self.current().map_or(SyntaxKind::Error, |t| t.kind)
    }

    fn bump(&mut self) {
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

    /// Consume the current token regardless of its kind, reporting
    /// an error if it does not match `expected`.  Always places the
    /// token into the CST so that downstream nodes are not lost.
    fn bump_expected(&mut self, expected: SyntaxKind) {
        if self.current_kind() != expected {
            self.error(format!(
                "expected {:?}, found {:?}",
                expected,
                self.current_kind()
            ));
        }
        if self.current().is_some() {
            self.bump();
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
        // Skip optional visibility
        let _vis = self.parse_visibility();

        match self.current_kind() {
            SyntaxKind::KwFn => self.parse_fn_def(),
            SyntaxKind::KwStruct => self.parse_struct_def(),
            SyntaxKind::KwEnum => self.parse_enum_def(),
            SyntaxKind::KwTrait => self.parse_trait_def(),
            SyntaxKind::KwImpl => self.parse_impl_def(),
            SyntaxKind::KwMod => {
                tracing::warn!("STUB: module parsing not yet implemented");
                self.bump(); // mod
                self.expect(SyntaxKind::Ident);
                if self.current_kind() == SyntaxKind::LBrace {
                    self.parse_block_inner();
                } else {
                    self.expect(SyntaxKind::Semicolon);
                }
            }
            SyntaxKind::KwConst => {
                tracing::warn!("STUB: const parsing not yet implemented");
                self.bump(); // const
                self.expect(SyntaxKind::Ident);
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
                self.expect(SyntaxKind::Ident);
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
                self.expect(SyntaxKind::Ident);
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
                // Error recovery: skip tokens until we find a likely item start or EOF
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
        // Handle &self, &mut self, self, mut self
        if self.current_kind() == SyntaxKind::And {
            self.bump(); // &
            if self.current_kind() == SyntaxKind::KwMut {
                self.bump(); // mut
            }
            if self.current_kind() == SyntaxKind::KwSelf {
                self.bump(); // self
                self.finish_node(); // Param
                return;
            }
            // & + ident
            self.expect(SyntaxKind::Ident);
            self.expect(SyntaxKind::Colon);
            self.parse_type();
        } else if self.current_kind() == SyntaxKind::KwMut {
            self.bump(); // mut
            if self.current_kind() == SyntaxKind::KwSelf {
                self.bump(); // self
                self.finish_node(); // Param
                return;
            }
            self.expect(SyntaxKind::Ident);
            self.expect(SyntaxKind::Colon);
            self.parse_type();
        } else if self.current_kind() == SyntaxKind::KwSelf {
            self.bump(); // self
            self.finish_node(); // Param
            return;
        } else {
            self.expect(SyntaxKind::Ident);
            self.expect(SyntaxKind::Colon);
            self.parse_type();
        }
        self.finish_node(); // Param
    }

    // ---- STRUCT ----

    fn parse_struct_def(&mut self) {
        self.start_node(SyntaxKind::StructDef);
        self.expect(SyntaxKind::KwStruct);
        self.expect(SyntaxKind::Ident);
        if self.current_kind() == SyntaxKind::Lt {
            self.parse_type_param_list();
        }
        match self.current_kind() {
            SyntaxKind::LParen => {
                // Tuple struct: just consume tokens until ');'
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
                // Record struct: skip field declarations
                self.bump(); // {
                while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
                    // parse field: ident : type
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
                // Unit struct
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
            self.expect(SyntaxKind::Ident);
            // Tuple variant: Red(i32)
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
            // Record variant: Green { r: u8, g: u8, b: u8 }
            if self.current_kind() == SyntaxKind::LBrace {
                self.bump(); // {
                while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
                    // parse field: ident : type
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
            // Discriminant: Red = 1
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
            // supertraits
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
            // Trait items: fn, type, const
            match self.current_kind() {
                SyntaxKind::KwFn => self.parse_fn_def(),
                SyntaxKind::KwType => {
                    tracing::warn!("STUB: associated type in trait");
                    self.bump(); // type
                    self.expect(SyntaxKind::Ident);
                    if self.current_kind() == SyntaxKind::Colon {
                        self.bump();
                        // bounds
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
                    self.expect(SyntaxKind::Ident);
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
        // Trait or inherent?
        
        // Parse the first type
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
                    self.expect(SyntaxKind::Ident);
                    if self.current_kind() == SyntaxKind::Eq {
                        self.bump();
                        self.parse_type();
                    }
                    self.expect(SyntaxKind::Semicolon);
                }
                SyntaxKind::KwConst => {
                    tracing::warn!("STUB: associated const in impl");
                    self.bump(); // const
                    self.expect(SyntaxKind::Ident);
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
        tracing::warn!("STUB: where clause parsing not fully implemented");
        self.start_node(SyntaxKind::WhereClause);
        self.expect(SyntaxKind::KwWhere);
        while self.current_kind() != SyntaxKind::LBrace
            && self.current_kind() != SyntaxKind::Semicolon
            && self.current().is_some()
        {
            self.bump();
        }
        self.finish_node();
    }

    // ---- TYPE PARAMS ----

    fn parse_type_param_list(&mut self) {
        self.start_node(SyntaxKind::TypeParamList);
        self.expect(SyntaxKind::Lt);
        while self.current_kind() != SyntaxKind::Gt && self.current().is_some() {
            self.start_node(SyntaxKind::TypeParam);
            self.expect(SyntaxKind::Ident);
            // Optional bounds
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
            self.finish_node(); // TypeParam
            if self.current_kind() == SyntaxKind::Comma {
                self.bump();
            }
        }
        self.expect(SyntaxKind::Gt);
        self.finish_node();
    }

    fn parse_type_arg_list(&mut self) {
        // Already inside a path type node; consume < and type args
        self.bump(); // <
        while self.current_kind() != SyntaxKind::Gt && self.current().is_some() {
            self.parse_type();
            if self.current_kind() == SyntaxKind::Comma {
                self.bump();
            }
        }
        self.expect(SyntaxKind::Gt);
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
                self.expect(SyntaxKind::KwLet);
                self.parse_pat();
                if self.current_kind() == SyntaxKind::Colon {
                    self.bump();
                    self.parse_type();
                }
                if self.current_kind() == SyntaxKind::Eq {
                    self.bump();
                    self.parse_expr();
                }
                // Always require semicolon, even before a closing brace.
                if self.current_kind() == SyntaxKind::Semicolon {
                    self.bump();
                } else {
                    self.error("expected ';' after let statement");
                    // Do not bump the unexpected token; let the next iteration recover.
                }
                self.finish_node();
            }
            SyntaxKind::KwIf
            | SyntaxKind::KwWhile
            | SyntaxKind::KwFor | SyntaxKind::KwLoop
            | SyntaxKind::KwReturn
            | SyntaxKind::KwBreak
            | SyntaxKind::KwContinue => {
                self.start_node(SyntaxKind::ExprStmt);
                self.parse_expr();
                if self.current_kind() == SyntaxKind::Semicolon {
                    self.bump();
                }
                self.finish_node();
            }
            SyntaxKind::KwFn => {
                // Item-level fn inside block (unusual but allowed in some languages)
                self.parse_fn_def();
            }
            SyntaxKind::KwStruct | SyntaxKind::KwEnum => {
                self.parse_item();
            }
            SyntaxKind::LBrace => {
                // Block expression as statement
                self.start_node(SyntaxKind::ExprStmt);
                self.parse_block();
                self.finish_node();
            }
            _ => {
                // Before parsing as expression, check if token is a known expression starter.
                // If not, report error and skip to avoid infinite loops.
                if !matches!(self.current_kind(),
                    SyntaxKind::Ident | SyntaxKind::IntLit | SyntaxKind::FloatLit |
                    SyntaxKind::StringLit | SyntaxKind::CharLit | SyntaxKind::KwTrue |
                    SyntaxKind::KwFalse | SyntaxKind::KwSelf | SyntaxKind::KwSuper |
                    SyntaxKind::KwCrate | SyntaxKind::LParen | SyntaxKind::LBrace |
                    SyntaxKind::KwIf | SyntaxKind::KwWhile | SyntaxKind::KwFor |
                    SyntaxKind::KwLoop | SyntaxKind::KwMatch | SyntaxKind::KwReturn |
                    SyntaxKind::KwBreak | SyntaxKind::KwContinue | SyntaxKind::Bang |
                    SyntaxKind::Minus | SyntaxKind::Star | SyntaxKind::And |
                    SyntaxKind::KwUnsafe
                ) {
                    self.error(format!("unexpected token in statement: {:?}", self.current_kind()));
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
                    // Expression without semicolon at end of block (e.g., tail expression)
                    // This is fine - the value is the return expression
                } else {
                    // Error recovery: missing semicolon
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
        self.parse_range_expr();
        if self.current_kind() == SyntaxKind::Eq {
            self.bump();
            let _prec = self.finish_node_hack();
            // Start AssignExpr, re-insert LHS as child? No - we'll just
            // wrap: the current implementation outputs a flat sequence.
            // For now, emit AssignExpr around the whole thing
            self.parse_assignment_expr();
        } else if matches!(
            self.current_kind(),
            SyntaxKind::PlusEq
                | SyntaxKind::MinusEq
                | SyntaxKind::StarEq
                | SyntaxKind::SlashEq
                | SyntaxKind::AndAnd
        ) {
            // Compound assignment - just bump and parse RHS for now
            // Full AST rewriting would require checkpoint/rewind
            self.bump();
            self.parse_assignment_expr();
        }
    }

    // HACK: finish_node but tell caller we did (for assignment wrapping)
    fn finish_node_hack(&mut self) {
        // This is intentionally a no-op; the actual tree restructuring
        // for assignments requires checkpoint/rewind which we don't have yet.
        // The diagnostic-free parse is the priority for v0.1.0.
    }

    fn parse_range_expr(&mut self) {
        self.parse_or_expr();
        if matches!(self.current_kind(), SyntaxKind::DotDot | SyntaxKind::DotDotEq) {
            self.bump(); // .. or ..=
            if !matches!(self.current_kind(),
                SyntaxKind::Eq | SyntaxKind::Semicolon | SyntaxKind::Comma |
                SyntaxKind::RParen | SyntaxKind::RBrace | SyntaxKind::RBracket)
            {
                self.parse_or_expr();
            }
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
        self.parse_additive_expr();
        if matches!(
            self.current_kind(),
            SyntaxKind::EqEq
                | SyntaxKind::BangEq
                | SyntaxKind::Lt
                | SyntaxKind::Gt
                | SyntaxKind::LtEq
                | SyntaxKind::GtEq
        ) {
            self.start_node(SyntaxKind::BinaryExpr);
            self.bump();
            self.parse_additive_expr();
            self.finish_node();
        }
    }

    fn parse_additive_expr(&mut self) {
        self.parse_multiplicative_expr();
        while matches!(self.current_kind(), SyntaxKind::Plus | SyntaxKind::Minus) {
            self.start_node(SyntaxKind::BinaryExpr);
            self.bump();
            self.parse_multiplicative_expr();
            self.finish_node();
        }
    }

    fn parse_multiplicative_expr(&mut self) {
        self.parse_unary_expr();
        while matches!(
            self.current_kind(),
            SyntaxKind::Star | SyntaxKind::Slash | SyntaxKind::Percent
        ) {
            self.start_node(SyntaxKind::BinaryExpr);
            self.bump();
            self.parse_unary_expr();
            self.finish_node();
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
        self.parse_primary_expr();

        // Postfix operations: method calls, field access, indexing, function calls
        loop {
            match self.current_kind() {
                SyntaxKind::Dot => {
                    self.bump(); // .
                    if self.current_kind() == SyntaxKind::Ident {
                        self.bump(); // field/method name
                    } else if self.current_kind() == SyntaxKind::IntLit
                        || self.current_kind() == SyntaxKind::FloatLit
                    {
                        // Tuple field access: expr.0
                        self.bump();
                    } else {
                        self.error("expected field name or index after '.'");
                    }
                    // After .field, check for turbofish or args
                    if self.current_kind() == SyntaxKind::ColonColon {
                        // Turbofish: expr.method::<Type>()
                        self.bump(); // ::
                        if self.current_kind() == SyntaxKind::Lt {
                            self.parse_type_param_list();
                        }
                    }
                    // Method call: expr.method()
                    if self.current_kind() == SyntaxKind::LParen {
                        self.bump(); // (
                        if self.current_kind() != SyntaxKind::RParen {
                            self.parse_expr();
                            while self.current_kind() == SyntaxKind::Comma {
                                self.bump();
                                self.parse_expr();
                            }
                        }
                        self.expect(SyntaxKind::RParen);
                    }
                }
                SyntaxKind::LParen => {
                    // Function call
                    self.bump(); // (
                    if self.current_kind() != SyntaxKind::RParen {
                        self.parse_expr();
                        while self.current_kind() == SyntaxKind::Comma {
                            self.bump();
                            self.parse_expr();
                        }
                    }
                    self.expect(SyntaxKind::RParen);
                }
                SyntaxKind::LBracket => {
                    // Index expression: expr[index]
                    self.bump(); // [
                    self.parse_expr();
                    self.expect(SyntaxKind::RBracket);
                }
                SyntaxKind::Question => {
                    // Try operator: expr?
                    self.bump();
                }
                SyntaxKind::KwAs => {
                    // Cast: expr as Type
                    self.bump();
                    self.parse_type();
                }
                _ => break,
            }
        }
    }

    fn parse_primary_expr(&mut self) {
        match self.current_kind() {
            SyntaxKind::Ident
            | SyntaxKind::KwSelf
            | SyntaxKind::KwSuper
            | SyntaxKind::KwCrate => {
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
                self.bump(); // (
                if self.current_kind() == SyntaxKind::RParen {
                    // Empty tuple
                    self.bump();
                } else {
                    self.parse_expr();
                    // Could be tuple: (a, b)
                    while self.current_kind() == SyntaxKind::Comma {
                        self.bump();
                        self.parse_expr();
                    }
                    self.expect(SyntaxKind::RParen);
                }
            }
            SyntaxKind::LBrace => self.parse_block(),
            SyntaxKind::KwIf => self.parse_if_expr(),
            SyntaxKind::KwWhile => self.parse_while_expr(),
            SyntaxKind::KwLoop => self.parse_loop_expr(),
            SyntaxKind::KwFor => self.parse_for_expr(),
            SyntaxKind::KwReturn => {
                self.bump(); // return
                if !matches!(
                    self.current_kind(),
                    SyntaxKind::Semicolon | SyntaxKind::RBrace
                ) {
                    self.parse_expr();
                }
            }
            SyntaxKind::KwBreak => {
                self.bump(); // break
                if !matches!(
                    self.current_kind(),
                    SyntaxKind::Semicolon | SyntaxKind::RBrace
                ) {
                    self.parse_expr();
                }
            }
            SyntaxKind::KwContinue => {
                self.bump(); // continue
            }
            SyntaxKind::KwMatch => {
                self.bump(); // match
                self.parse_expr();
                self.expect(SyntaxKind::LBrace);
                while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
                    self.parse_match_arm();
                }
                self.expect(SyntaxKind::RBrace);
            }
            _ => {
                self.error(format!(
                    "expected expression, found {:?}",
                    self.current_kind()
                ));
                // CRITICAL: always consume token to prevent infinite loop
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
    }

    fn parse_path(&mut self) {
        self.start_node(SyntaxKind::UsePath);
        // First segment
        match self.current_kind() {
            SyntaxKind::Ident
            | SyntaxKind::KwSelf
            | SyntaxKind::KwSuper
            | SyntaxKind::KwCrate => {
                self.bump();
            }
            _ => {
                self.error("expected identifier in path");
                return;
            }
        }
        // Subsequent segments with :: (module paths) or . (field access)
        loop {
            match self.current_kind() {
                SyntaxKind::ColonColon => {
                    self.bump(); // ::
                    match self.current_kind() {
                        SyntaxKind::Ident
                        | SyntaxKind::KwSelf
                        | SyntaxKind::KwSuper => {
                            self.bump();
                        }
                        SyntaxKind::Lt => {
                            // turbofish: path::<Type>
                            self.parse_type_param_list();
                        }
                        _ => {
                            self.error("expected identifier after '::'");
                            break;
                        }
                    }
                }
                SyntaxKind::Dot => {
                    // Field access: a.b
                    self.bump(); // .
                    if self.current_kind() == SyntaxKind::Ident {
                        self.bump();
                    } else {
                        self.error("expected field name after '.'");
                        // break? Keep going for error recovery
                    }
                }
                _ => break,
            }
        }
        self.finish_node();
    }

    fn parse_if_expr(&mut self) {
        self.start_node(SyntaxKind::IfExpr);
        self.expect(SyntaxKind::KwIf);
        // Check for if-let: if let Pat = Expr
        if self.current_kind() == SyntaxKind::KwLet {
            self.bump(); // let
            self.parse_pat();
            self.expect(SyntaxKind::Eq);
            self.parse_expr();
        } else {
            self.parse_expr();
        }
        self.parse_block();
        if self.current_kind() == SyntaxKind::KwElse {
            self.bump(); // else
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
        // Check for while-let: while let Pat = Expr
        if self.current_kind() == SyntaxKind::KwLet {
            self.bump(); // let
            self.parse_pat();
            self.expect(SyntaxKind::Eq);
            self.parse_expr();
        } else {
            self.parse_expr();
        }
        self.parse_block();
        self.finish_node();
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
        tracing::warn!("STUB: for expression parsing");
        self.bump(); // for
        self.parse_pat();
        self.expect(SyntaxKind::KwIn);
        self.parse_expr();
        self.parse_block();
    }

    fn parse_match_arm(&mut self) {
        self.start_node(SyntaxKind::MatchArm);
        self.parse_pat();
        if self.current_kind() == SyntaxKind::KwIf {
            self.bump(); // if guard
            self.parse_expr();
        }
        self.expect(SyntaxKind::FatArrow);
        // Arm body: either expression (with comma) or block
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
        // Handle or-patterns: A | B
        self.parse_pat_single();
        while self.current_kind() == SyntaxKind::Or {
            self.bump(); // |
            self.parse_pat_single();
        }
    }

    fn parse_pat_single(&mut self) {
        match self.current_kind() {
            SyntaxKind::KwRef => {
                self.bump(); // ref
                if self.current_kind() == SyntaxKind::KwMut {
                    self.bump(); // mut
                }
                // parse inner pattern (no PatRef node)
                self.parse_pat_inner();
            }
            SyntaxKind::KwMut => {
                self.bump(); // mut
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
                self.finish_node(); // PatTuple
            }
            _ => {
                self.parse_pat_inner();
            }
        }
    }

    fn parse_pat_inner(&mut self) {
        match self.current_kind() {
            SyntaxKind::Underscore => {
                self.start_node(SyntaxKind::PatWild);
                self.bump();
                self.finish_node();
            }
            SyntaxKind::Ident | SyntaxKind::KwSelf => {
                self.start_node(SyntaxKind::PatIdent);
                self.bump();
                self.finish_node();
            }
            SyntaxKind::IntLit
            | SyntaxKind::FloatLit
            | SyntaxKind::StringLit
            | SyntaxKind::CharLit
            | SyntaxKind::KwTrue
            | SyntaxKind::KwFalse => {
                self.start_node(SyntaxKind::PatLit);
                self.bump();
                self.finish_node();
            }
            _ => {
                self.error(format!("expected pattern, found {:?}", self.current_kind()));
                // Consume token to prevent infinite loop
                if self.current().is_some() {
                    self.bump();
                }
            }
        }
    }


    // ---- TYPES ----

    fn parse_type(&mut self) {
        match self.current_kind() {
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
                if self.current_kind() == SyntaxKind::KwConst || self.current_kind() == SyntaxKind::KwMut {
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
                    self.parse_expr(); // const expr for size
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
            SyntaxKind::Underscore => {
                self.start_node(SyntaxKind::InferType);
                self.bump();
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
            SyntaxKind::Ident
            | SyntaxKind::KwSelf
            | SyntaxKind::KwSuper
            | SyntaxKind::KwCrate => {
                self.start_node(SyntaxKind::PathType);
                self.parse_path();
                // Optional generic type arguments: Option<i32>
                if self.current_kind() == SyntaxKind::Lt {
                    self.parse_type_arg_list();
                }
                self.finish_node();
            }
            _ => {
                self.error(format!("expected type, found {:?}", self.current_kind()));
                // Consume token to prevent infinite loop
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
