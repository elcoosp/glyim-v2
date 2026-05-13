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

// struct Checkpoint { inner: rowan::Checkpoint }

struct Parser<'a> {
    tokens: &'a [crate::lexer::Token],
    pos: usize,
    builder: rowan::GreenNodeBuilder<'a>,
    diagnostics: Vec<GlyimDiagnostic>,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [crate::lexer::Token]) -> Self {
        Self {
            tokens,
            pos: 0,
            builder: rowan::GreenNodeBuilder::new(),
            diagnostics: Vec::new(),
        }
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

    fn start_node(&mut self, kind: SyntaxKind) {
        let raw_kind = GlyimLang::kind_to_raw(kind);
        self.builder.start_node(raw_kind);
    }
    fn finish_node(&mut self) {
        self.builder.finish_node();
    }

    //    fn checkpoint(&self) -> Checkpoint { Checkpoint { inner: self.builder.checkpoint() } }

    fn parse_source_file(&mut self) {
        self.start_node(SyntaxKind::SourceFile);
        while self.current().is_some() {
            self.parse_item();
        }
        self.finish_node();
    }

    fn parse_item(&mut self) {
        match self.current_kind() {
            SyntaxKind::KwFn => self.parse_fn_def(),
            SyntaxKind::KwStruct => self.parse_struct_def(),
            SyntaxKind::KwEnum => self.parse_enum_def(),
            _ => {
                self.error("expected item");
                self.bump();
            }
        }
    }

    fn parse_fn_def(&mut self) {
        self.start_node(SyntaxKind::FnDef);
        self.expect(SyntaxKind::KwFn);
        self.expect(SyntaxKind::Ident);
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
        self.finish_node();
        self.expect(SyntaxKind::RParen);
        if self.current_kind() == SyntaxKind::Arrow {
            self.bump();
            self.parse_type();
        }
        if self.current_kind() == SyntaxKind::LBrace {
            self.parse_block();
        }
        self.finish_node();
    }

    fn parse_struct_def(&mut self) {
        self.start_node(SyntaxKind::StructDef);
        self.expect(SyntaxKind::KwStruct);
        self.expect(SyntaxKind::Ident);
        if self.current_kind() == SyntaxKind::Lt {
            self.parse_type_param_list();
        }
        self.expect(SyntaxKind::Semicolon);
        self.finish_node();
    }

    fn parse_enum_def(&mut self) {
        self.start_node(SyntaxKind::EnumDef);
        self.expect(SyntaxKind::KwEnum);
        self.expect(SyntaxKind::Ident);
        if self.current_kind() == SyntaxKind::Lt {
            self.parse_type_param_list();
        }
        self.expect(SyntaxKind::LBrace);
        self.start_node(SyntaxKind::VariantList);
        while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
            self.start_node(SyntaxKind::EnumVariant);
            self.expect(SyntaxKind::Ident);
            self.finish_node();
            if self.current_kind() == SyntaxKind::Comma {
                self.bump();
            }
        }
        self.finish_node();
        self.expect(SyntaxKind::RBrace);
        self.finish_node();
    }

    fn parse_type_param_list(&mut self) {
        self.start_node(SyntaxKind::TypeParamList);
        self.expect(SyntaxKind::Lt);
        while self.current_kind() != SyntaxKind::Gt && self.current().is_some() {
            self.start_node(SyntaxKind::TypeParam);
            self.expect(SyntaxKind::Ident);
            self.finish_node();
            if self.current_kind() == SyntaxKind::Comma {
                self.bump();
            }
        }
        self.expect(SyntaxKind::Gt);
        self.finish_node();
    }

    fn parse_param(&mut self) {
        self.start_node(SyntaxKind::Param);
        self.expect(SyntaxKind::Ident);
        self.expect(SyntaxKind::Colon);
        self.parse_type();
        self.finish_node();
    }

    fn parse_block(&mut self) {
        self.start_node(SyntaxKind::Block);
        self.expect(SyntaxKind::LBrace);
        while self.current_kind() != SyntaxKind::RBrace && self.current().is_some() {
            self.parse_stmt();
        }
        self.expect(SyntaxKind::RBrace);
        self.finish_node();
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
                self.expect(SyntaxKind::Semicolon);
                self.finish_node();
            }
            _ => {
                self.start_node(SyntaxKind::ExprStmt);
                self.parse_expr();
                if self.current_kind() == SyntaxKind::Semicolon {
                    self.bump();
                }
                self.finish_node();
            }
        }
    }

    fn parse_expr(&mut self) {
        self.parse_assignment_expr();
    }

    fn parse_assignment_expr(&mut self) {
        self.parse_or_expr();
        if self.current_kind() == SyntaxKind::Eq {
            self.start_node(SyntaxKind::AssignExpr);
            self.bump();
            self.parse_expr();
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
            self.parse_primary_expr();
        }
    }

    fn parse_primary_expr(&mut self) {
        match self.current_kind() {
            SyntaxKind::Ident => self.parse_path_expr(),
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
                self.bump();
                if self.current_kind() == SyntaxKind::RParen {
                    self.bump();
                } else {
                    self.parse_expr();
                    self.expect(SyntaxKind::RParen);
                }
            }
            SyntaxKind::LBrace => self.parse_block(),
            SyntaxKind::KwIf => self.parse_if_expr(),
            SyntaxKind::KwReturn => {
                self.bump();
                if !matches!(
                    self.current_kind(),
                    SyntaxKind::Semicolon | SyntaxKind::RBrace
                ) {
                    self.parse_expr();
                }
            }
            _ => {
                self.error("expected expression");
                self.bump();
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
        self.expect(SyntaxKind::Ident);
        while self.current_kind() == SyntaxKind::ColonColon {
            self.bump();
            self.expect(SyntaxKind::Ident);
        }
        self.finish_node();
    }

    fn parse_if_expr(&mut self) {
        self.start_node(SyntaxKind::IfExpr);
        self.expect(SyntaxKind::KwIf);
        self.parse_expr();
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

    fn parse_pat(&mut self) {
        match self.current_kind() {
            SyntaxKind::Underscore => {
                self.start_node(SyntaxKind::PatWild);
                self.bump();
                self.finish_node();
            }
            SyntaxKind::Ident => {
                self.start_node(SyntaxKind::PatIdent);
                self.bump();
                self.finish_node();
            }
            _ => {
                self.error("expected pattern");
                self.bump();
            }
        }
    }

    fn parse_type(&mut self) {
        match self.current_kind() {
            SyntaxKind::Ident => {
                self.start_node(SyntaxKind::PathType);
                self.parse_path();
                self.finish_node();
            }
            _ => {
                self.error("expected type");
                self.bump();
            }
        }
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

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
