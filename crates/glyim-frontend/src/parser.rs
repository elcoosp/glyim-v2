use glyim_syntax::{SyntaxKind, SyntaxNode, GreenNode, GlyimLang};
use rowan::Language;
use glyim_span::FileId;
use glyim_diag::GlyimDiagnostic;

#[derive(Clone, Debug)]
pub struct ParseResult {
    pub green_node: GreenNode,
    pub diagnostics: Vec<GlyimDiagnostic>,
    pub root: SyntaxNode,
}

#[tracing::instrument(skip(source))]
pub fn parse_to_syntax(source: &str, file_id: FileId) -> ParseResult {
    let _ = (source, file_id);
    let mut builder = rowan::GreenNodeBuilder::new();
    // kind_to_raw already returns rowan::SyntaxKind
    let kind = GlyimLang::kind_to_raw(SyntaxKind::SourceFile);
    builder.start_node(kind);
    builder.finish_node();
    let green_node = builder.finish();
    let root = SyntaxNode::new_root(green_node.clone());
    ParseResult { green_node, diagnostics: Vec::new(), root }
}
