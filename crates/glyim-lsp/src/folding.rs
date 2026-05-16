use crate::AnalysisDatabase;
use lsp_types::*;

pub fn provide_folding_ranges(
    _db: &AnalysisDatabase,
    _params: &FoldingRangeParams,
) -> Option<Vec<FoldingRange>> {
    tracing::warn!("STUB: provide_folding_ranges");
    None
}
