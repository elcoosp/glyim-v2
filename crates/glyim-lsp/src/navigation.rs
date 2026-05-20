#![allow(deprecated)]

use crate::AnalysisDatabase;
use crate::database::FileMap;
use lsp_types::*;
use url::Url;

fn get_symbol_name_at_position(
    db: &AnalysisDatabase,
    file_map: &FileMap,
    uri: &Url,
    position: Position,
) -> Option<String> {
    let path = uri.to_file_path().ok()?;
    let file_id = file_map.get_by_path(&path)?;
    let source_maps = db.source_maps.read();
    let sm = source_maps.get(&file_id)?;
    let offset = sm.line_col_to_offset(position.line as usize, position.character as usize)?;
    let source = sm.source();
    let chars: Vec<char> = source.chars().collect();
    let mut start = offset;
    let mut end = offset;
    while start > 0 && (chars[start - 1].is_alphabetic() || chars[start - 1] == '_') {
        start -= 1;
    }
    while end < chars.len() && (chars[end].is_alphabetic() || chars[end] == '_') {
        end += 1;
    }
    if start == end {
        None
    } else {
        Some(source[start..end].to_string())
    }
}

pub fn goto_definition(
    db: &AnalysisDatabase,
    file_map: &FileMap,
    params: &GotoDefinitionParams,
) -> Option<GotoDefinitionResponse> {
    let uri = &params.text_document_position_params.text_document.uri;
    let path = uri.to_file_path().ok()?;
    let file_id = file_map.get_by_path(&path)?;
    let source_maps = db.source_maps.read();
    let sm = source_maps.get(&file_id)?;
    let pos = params.text_document_position_params.position;
    let offset = sm.line_col_to_offset(pos.line as usize, pos.character as usize)?;
    let symbol_index = db.symbol_index.read();
    let symbol = symbol_index.lookup_by_location(file_id, offset)?;
    let def = &symbol.definition;
    let def_sm = source_maps.get(&def.file_id)?;
    let (start_line, start_col) = def_sm
        .span_to_position(def.span.lo.to_usize(), def.span.hi.to_usize())
        .unwrap_or(((0, 0), (0, 0)))
        .0;
    let target_path = file_map.path(def.file_id)?;
    let target_uri = Url::from_file_path(target_path).ok()?;
    Some(GotoDefinitionResponse::Scalar(Location {
        uri: target_uri,
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
    }))
}

pub fn find_references(
    db: &AnalysisDatabase,
    file_map: &FileMap,
    params: &ReferenceParams,
) -> Option<Vec<Location>> {
    let uri = &params.text_document_position.text_document.uri;
    let symbol_name =
        get_symbol_name_at_position(db, file_map, uri, params.text_document_position.position)?;
    let ref_graph = db.reference_graph.read();
    let references = ref_graph.find_references(&symbol_name);
    if references.is_empty() {
        return None;
    }
    let source_maps = db.source_maps.read();
    let mut locations = Vec::new();
    for r in references {
        let sm = source_maps.get(&r.file_id)?;
        let (start_line, start_col) = sm
            .span_to_position(r.span.lo.to_usize(), r.span.hi.to_usize())
            .unwrap_or(((0, 0), (0, 0)))
            .0;
        let path = file_map.path(r.file_id)?;
        let loc_uri = Url::from_file_path(path).ok()?;
        locations.push(Location {
            uri: loc_uri,
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
        });
    }
    Some(locations)
}

pub fn document_symbols(
    db: &AnalysisDatabase,
    file_map: &FileMap,
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
            deprecated: None,
        });
    }
    Some(results)
}
