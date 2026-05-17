use crate::AnalysisDatabase;
use lsp_types::*;

pub fn find_references(
    _db: &AnalysisDatabase,
    _file_map: &crate::database::FileMap,
    _params: &ReferenceParams,
) -> Option<Vec<Location>> {
    None
}

pub fn document_symbols(
    db: &AnalysisDatabase,
    file_map: &crate::database::FileMap,
    params: &DocumentSymbolParams,
) -> Option<DocumentSymbolResponse> {
    let uri = &params.text_document.uri;
    let path = uri.to_file_path().ok()?;
    let file_id = file_map.get_by_path(&path)?;
    let source_maps = db.source_maps.read();
    let sm = source_maps.get(&file_id)?;
    let symbol_index = db.symbol_index.read();
    let symbols = symbol_index.symbols_in_file(file_id);
    let mut results = Vec::new();
    for sym in symbols {
        let (start_line, start_col) = sm
            .span_to_position(
                sym.definition.span.lo.to_usize(),
                sym.definition.span.hi.to_usize(),
            )
            .unwrap_or(((0, 0), (0, 0)))
            .0;
        let kind = match sym.kind {
            crate::symbol_index::SymbolKind::Function => SymbolKind::FUNCTION,
            crate::symbol_index::SymbolKind::Struct => SymbolKind::STRUCT,
            crate::symbol_index::SymbolKind::Enum => SymbolKind::ENUM,
            crate::symbol_index::SymbolKind::Field => SymbolKind::FIELD,
            crate::symbol_index::SymbolKind::Local => SymbolKind::VARIABLE,
            _ => SymbolKind::VARIABLE,
        };
        results.push(DocumentSymbol {
            name: sym.name.clone(),
            kind,
            range: Range {
                start: Position {
                    line: start_line as u32,
                    character: start_col as u32,
                },
                end: Position {
                    line: start_line as u32,
                    character: (start_col + 1) as u32,
                },
            },
            selection_range: Range {
                start: Position {
                    line: start_line as u32,
                    character: start_col as u32,
                },
                end: Position {
                    line: start_line as u32,
                    character: start_col as u32,
                },
            },
            children: None,
            detail: sym.type_signature.as_ref().map(|ts| {
                let params: Vec<String> = ts
                    .params
                    .iter()
                    .map(|(n, t)| format!("{}: {}", n, t))
                    .collect();
                format!("({})", params.join(", "))
            }),
            tags: None,
            #[allow(deprecated)]
            deprecated: None,
        });
    }
    Some(DocumentSymbolResponse::Nested(results))
}

pub fn workspace_symbols(
    db: &AnalysisDatabase,
    params: &WorkspaceSymbolParams,
) -> Option<Vec<SymbolInformation>> {
    let query = params.query.as_str();
    let symbol_index = db.symbol_index.read();
    let matches = symbol_index.query(query, 20);
    let source_maps = db.source_maps.read();
    let file_map = db.file_map.read();
    let mut results = Vec::new();
    for info in matches {
        let sm = source_maps.get(&info.definition.file_id)?;
        let (start_line, start_col) = sm
            .span_to_position(
                info.definition.span.lo.to_usize(),
                info.definition.span.hi.to_usize(),
            )
            .unwrap_or(((0, 0), (0, 0)))
            .0;
        let path = file_map.path(info.definition.file_id)?;
        let uri = Url::from_file_path(path).ok()?;
        let kind = match info.kind {
            crate::symbol_index::SymbolKind::Function => SymbolKind::FUNCTION,
            crate::symbol_index::SymbolKind::Struct => SymbolKind::STRUCT,
            crate::symbol_index::SymbolKind::Enum => SymbolKind::ENUM,
            crate::symbol_index::SymbolKind::Field => SymbolKind::FIELD,
            _ => SymbolKind::VARIABLE,
        };
        results.push(SymbolInformation {
            name: info.name.clone(),
            kind,
            location: Location {
                uri,
                range: Range {
                    start: Position {
                        line: start_line as u32,
                        character: start_col as u32,
                    },
                    end: Position {
                        line: start_line as u32,
                        character: (start_col + 1) as u32,
                    },
                },
            },
            container_name: None,
            tags: None,
            #[allow(deprecated)]
            deprecated: None,
        });
    }
    Some(results)
}

pub fn rename(_db: &AnalysisDatabase, _params: &RenameParams) -> Option<WorkspaceEdit> {
    None
}
