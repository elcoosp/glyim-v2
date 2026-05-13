use glyim_syntax::SyntaxKind;
use glyim_span::{ByteIdx, FileId, Span, SyntaxContext};
use glyim_diag::GlyimDiagnostic;
use smol_str::SmolStr;

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: SyntaxKind,
    pub span: Span,
    pub text: SmolStr,
}

impl Token {
    pub fn new(kind: SyntaxKind, span: Span, text: impl AsRef<str>) -> Self {
        Self { kind, span, text: SmolStr::from(text.as_ref()) }
    }
}

#[derive(Clone, Debug)]
pub struct LexResult {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<GlyimDiagnostic>,
}

#[tracing::instrument(skip(source))]
pub fn lex(source: &str, file_id: FileId) -> LexResult {
    // STUB: Incomplete implementation – will be fleshed out later
    LexResult { tokens: Vec::new(), diagnostics: Vec::new() }
}
