use crate::AnalysisDatabase;
use crate::database::FileMap;
use lsp_types::*;
use url::Url;

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
        return None;
    }
    let symbol_name = &source[start..end];

    let symbol_index = db.symbol_index.read();
    let symbols = symbol_index.lookup_by_name(symbol_name);
    let symbol = symbols.first()?;
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
