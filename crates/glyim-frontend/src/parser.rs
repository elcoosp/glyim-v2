use glyim_syntax::{SyntaxKind, SyntaxNode, GreenNode, GlyimLang};
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
    // STUB: produce an empty source file
    let mut builder = rowan::GreenNodeBuilder::new();
    builder.start_node(rowan::SyntaxKind(GlyimLang::kind_to_raw(SyntaxKind::SourceFile)));
    builder.finish_node();
    let green_node = builder.finish();
    let root = SyntaxNode::new_root(green_node.clone());
    ParseResult { green_node, diagnostics: Vec::new(), root }
}
