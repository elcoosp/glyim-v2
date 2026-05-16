use crate::AnalysisDatabase;
use lsp_types::*;

pub fn provide_code_actions(
    _db: &AnalysisDatabase,
    _params: &CodeActionParams,
) -> Option<Vec<CodeActionOrCommand>> {
    tracing::warn!("STUB: provide_code_actions");
    None
}
