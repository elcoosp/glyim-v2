use glyim_diag::GlyimDiagnostic;
use glyim_span::FileId;
use glyim_syntax::{GlyimLang, GreenNode, SyntaxKind, SyntaxNode};
use rowan::Language;

// Submodules containing specific parsing logic
mod expr;
mod item;
mod pat;
mod stmt;
mod ty;

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

    // Dispatchers to submodules
    // Implementations are in the submodules.

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
