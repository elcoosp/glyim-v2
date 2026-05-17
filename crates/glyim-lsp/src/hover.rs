use crate::AnalysisDatabase;
use lsp_types::*;

pub fn provide_hover(
    db: &AnalysisDatabase,
    file_map: &crate::database::FileMap,
    params: &HoverParams,
) -> Option<Hover> {
    let uri = &params.text_document_position_params.text_document.uri;
    let path = uri.to_file_path().ok()?;
    let file_id = file_map.get_by_path(&path)?;
    let source_maps = db.source_maps.read();
    let sm = source_maps.get(&file_id)?;
    let pos = params.text_document_position_params.position;
    let offset = sm.line_col_to_offset(pos.line as usize, pos.character as usize)?;
    let symbol_index = db.symbol_index.read();
    let symbol = symbol_index.lookup_by_location(file_id, offset)?;
    let mut markdown = String::new();
    if let Some(ts) = &symbol.type_signature {
        let params_str: Vec<String> = ts
            .params
            .iter()
            .map(|(n, t)| format!("{}: {}", n, t))
            .collect();
        let ret_str = ts
            .return_type
            .as_ref()
            .map(|t| format!(" -> {}", t))
            .unwrap_or_default();
        markdown.push_str(&format!(
            "```glyim\nfn {}({}){}\n```\n",
            symbol.name,
            params_str.join(", "),
            ret_str
        ));
    }
    if let Some(doc) = &symbol.documentation {
        markdown.push_str(doc);
    }
    if markdown.is_empty() {
        return None;
    }
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: markdown,
        }),
        range: None,
    })
}
