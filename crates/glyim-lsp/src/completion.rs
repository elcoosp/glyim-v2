use crate::AnalysisDatabase;
use crate::symbol_index::{SymbolInfo, SymbolKind};
use glyim_type::PrintTy;
use lsp_types::*;

use url::Url;

/// Build a single `CompletionItem` from an indexed `SymbolInfo`.
fn completion_item(sym: &SymbolInfo) -> CompletionItem {
    let kind = match sym.kind {
        SymbolKind::Function => CompletionItemKind::FUNCTION,
        SymbolKind::Struct => CompletionItemKind::STRUCT,
        SymbolKind::Enum => CompletionItemKind::ENUM,
        SymbolKind::Field => CompletionItemKind::FIELD,
        SymbolKind::Local => CompletionItemKind::VARIABLE,
        _ => CompletionItemKind::TEXT,
    };
    let detail = sym.type_signature.as_ref().map(|ts| {
        let params: Vec<String> = ts
            .params
            .iter()
            .map(|(n, t)| format!("{}: {}", n, t))
            .collect();
        let ret = ts
            .return_type
            .as_ref()
            .map(|t| format!(" -> {}", t))
            .unwrap_or_default();
        format!("({}){}", params.join(", "), ret)
    });
    let documentation = sym.documentation.as_ref().map(|d| {
        Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: d.clone(),
        })
    });
    CompletionItem {
        label: sym.name.clone(),
        kind: Some(kind),
        detail,
        documentation,
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        insert_text: if sym.kind == SymbolKind::Function {
            sym.type_signature.as_ref().map(|ts| {
                if ts.params.is_empty() {
                    format!("{}()", sym.name)
                } else {
                    let placeholders: Vec<String> = ts
                        .params
                        .iter()
                        .enumerate()
                        .map(|(i, (n, _))| format!("${{{}:{}}}", i + 1, n))
                        .collect();
                    format!("{}({})", sym.name, placeholders.join(", "))
                }
            })
        } else {
            None
        },
        sort_text: Some(match sym.kind {
            SymbolKind::Function => format!("0_{}", sym.name),
            SymbolKind::Struct => format!("1_{}", sym.name),
            _ => format!("9_{}", sym.name),
        }),
        ..Default::default()
    }
}

/// If the cursor is positioned right after a `.` (a method-call completion
/// trigger), resolve the type of the receiver expression and return it as a
/// normalized string (e.g. `"Foo"`). Returns `None` when there is no `.`
/// trigger, no source map, or the receiver type cannot be resolved — in which
/// case callers should fall back to unfiltered completion.
fn receiver_type_at_cursor(
    db: &AnalysisDatabase,
    file_id: glyim_span::FileId,
    params: &CompletionParams,
) -> Option<String> {
    // Only filter when the LSP explicitly reports a `.` trigger character.
    let is_dot_trigger = params
        .context
        .as_ref()
        .and_then(|c| c.trigger_character.as_deref())
        == Some(".");
    if !is_dot_trigger {
        return None;
    }

    let source_maps = db.source_maps.read();
    let sm = source_maps.get(&file_id)?;
    let pos = params.text_document_position.position;
    let offset = sm.line_col_to_offset(pos.line as usize, pos.character as usize)?;

    // Confirm the character immediately before the cursor is a `.`.
    let src = sm.source();
    if offset == 0 || !src[..offset].ends_with('.') {
        return None;
    }

    let ty = db.type_at_offset(file_id, offset)?;
    let ty_ctx = db.ty_ctx(file_id)?;
    Some(format!("{}", PrintTy::new(ty, ty_ctx.as_ref())))
}

pub fn provide_completions(
    db: &AnalysisDatabase,
    file_map: &crate::database::FileMap,
    params: &CompletionParams,
) -> Option<CompletionResponse> {
    let uri = &params.text_document_position.text_document.uri;
    let path = Url::parse(uri.as_str()).ok()?.to_file_path().ok()?;
    let file_id = file_map.get_by_path(&path)?;
    let symbol_index = db.symbol_index.read();
    let symbols = symbol_index.symbols_in_file(file_id);

    // Tier 6.4: filter completions by the receiver type at a `.`-method call
    // site. When a receiver type resolves, prefer methods whose recorded
    // receiver type matches; if nothing matches, fall back to the full list so
    // completion is never empty for an incomplete expression.
    let receiver_ty = receiver_type_at_cursor(db, file_id, params);
    let filtered: Vec<&SymbolInfo> = if let Some(rt) = &receiver_ty {
        let methods: Vec<&SymbolInfo> = symbols
            .iter()
            .filter(|sym| {
                sym.type_signature
                    .as_ref()
                    .and_then(|ts| ts.receiver_type.as_deref())
                    == Some(rt.as_str())
            })
            .cloned()
            .collect();
        if methods.is_empty() { symbols } else { methods }
    } else {
        symbols
    };

    let items: Vec<CompletionItem> = filtered.iter().map(|sym| completion_item(sym)).collect();
    if items.is_empty() {
        None
    } else {
        Some(CompletionResponse::List(CompletionList {
            is_incomplete: false,
            items,
        }))
    }
}
