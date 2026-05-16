use crate::AnalysisDatabase;
use lsp_types::*;

pub fn provide_completions(
    db: &AnalysisDatabase,
    file_map: &crate::database::FileMap,
    params: &CompletionParams,
) -> Option<CompletionResponse> {
    let uri = &params.text_document_position.text_document.uri;
    let path = uri.to_file_path().ok()?;
    let file_id = file_map.get_by_path(&path)?;
    let symbol_index = db.symbol_index.read();
    let symbols = symbol_index.symbols_in_file(file_id);
    let items: Vec<CompletionItem> = symbols.iter().map(|sym| {
        let kind = match sym.kind {
            crate::symbol_index::SymbolKind::Function => CompletionItemKind::FUNCTION,
            crate::symbol_index::SymbolKind::Struct => CompletionItemKind::STRUCT,
            crate::symbol_index::SymbolKind::Enum => CompletionItemKind::ENUM,
            crate::symbol_index::SymbolKind::Field => CompletionItemKind::FIELD,
            crate::symbol_index::SymbolKind::Local => CompletionItemKind::VARIABLE,
            _ => CompletionItemKind::TEXT,
        };
        let detail = sym.type_signature.as_ref().map(|ts| {
            let params: Vec<String> = ts.params.iter().map(|(n, t)| format!("{}: {}", n, t)).collect();
            let ret = ts.return_type.as_ref().map(|t| format!(" -> {}", t)).unwrap_or_default();
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
            insert_text: if sym.kind == crate::symbol_index::SymbolKind::Function {
                sym.type_signature.as_ref().map(|ts| {
                    if ts.params.is_empty() {
                        format!("{}()", sym.name)
                    } else {
                        let placeholders: Vec<String> = ts.params.iter().enumerate()
                            .map(|(i, (n, _))| format!("${{{}:{}}}", i + 1, n))
                            .collect();
                        format!("{}({})", sym.name, placeholders.join(", "))
                    }
                })
            } else {
                None
            },
            sort_text: Some(match sym.kind {
                crate::symbol_index::SymbolKind::Function => format!("0_{}", sym.name),
                crate::symbol_index::SymbolKind::Struct => format!("1_{}", sym.name),
                _ => format!("9_{}", sym.name),
            }),
            ..Default::default()
        }
    }).collect();
    if items.is_empty() {
        None
    } else {
        Some(CompletionResponse::List(CompletionList {
            is_incomplete: false,
            items,
        }))
    }
}
