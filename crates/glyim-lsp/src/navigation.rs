use crate::AnalysisDatabase;
use lsp_types::*;

pub fn goto_definition(
    _db: &AnalysisDatabase,
    _file_map: &crate::database::FileMap,
    _params: &GotoDefinitionParams,
) -> Option<GotoDefinitionResponse> {
    tracing::warn!("STUB: goto_definition");
    None
}

pub fn find_references(
    _db: &AnalysisDatabase,
    _file_map: &crate::database::FileMap,
    _params: &ReferenceParams,
) -> Option<Vec<Location>> {
    tracing::warn!("STUB: find_references");
    None
}

pub fn document_symbols(
    _db: &AnalysisDatabase,
    _file_map: &crate::database::FileMap,
    _params: &DocumentSymbolParams,
) -> Option<DocumentSymbolResponse> {
    tracing::warn!("STUB: document_symbols");
    None
}

pub fn workspace_symbols(
    _db: &AnalysisDatabase,
    _params: &WorkspaceSymbolParams,
) -> Option<Vec<SymbolInformation>> {
    tracing::warn!("STUB: workspace_symbols");
    None
}

pub fn rename(
    _db: &AnalysisDatabase,
    _params: &RenameParams,
) -> Option<WorkspaceEdit> {
    tracing::warn!("STUB: rename");
    None
}
