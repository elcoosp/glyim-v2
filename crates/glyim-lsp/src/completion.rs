use crate::AnalysisDatabase;
use lsp_types::*;

pub fn provide_completions(
    _db: &AnalysisDatabase,
    _file_map: &crate::database::FileMap,
    _params: &CompletionParams,
) -> Option<CompletionResponse> {
    tracing::warn!("STUB: provide_completions");
    None
}
