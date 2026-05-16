use crate::AnalysisDatabase;
use lsp_types::*;

pub fn provide_hover(
    _db: &AnalysisDatabase,
    _file_map: &crate::database::FileMap,
    _params: &HoverParams,
) -> Option<Hover> {
    tracing::warn!("STUB: provide_hover");
    None
}
