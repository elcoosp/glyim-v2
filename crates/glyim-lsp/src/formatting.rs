use crate::AnalysisDatabase;
use lsp_types::*;

pub fn format_document(
    _db: &AnalysisDatabase,
    _params: &DocumentFormattingParams,
) -> Option<Vec<TextEdit>> {
    tracing::warn!("STUB: format_document");
    None
}
