use crate::database::AnalysisDatabase;
use crate::navigation::workspace_symbols;
use crate::symbol_index::{SymbolInfo, SymbolKind, DefinitionLocation};
use crate::tests::helpers::make_span;
use glyim_span::{FileId};
use crate::database::SourceMap;
use std::path::PathBuf;
use lsp_types::*;

fn setup_test_db() -> AnalysisDatabase {
    let db = AnalysisDatabase::new();
    let file_id1 = FileId::from_raw(0);
    let file_id2 = FileId::from_raw(1);
    let path1 = PathBuf::from("/test/main.g");
    let path2 = PathBuf::from("/test/lib.g");
    {
        let mut fm = db.file_map.write();
        fm.get_or_create(&path1);
        fm.get_or_create(&path2);
    }
    {
        let mut sm = db.source_maps.write();
        sm.insert(file_id1, SourceMap::new(path1, file_id1, "fn calculate() {}".to_string()));
        sm.insert(file_id2, SourceMap::new(path2, file_id2, "fn calculator() {}".to_string()));
    }
    {
        let mut idx = db.symbol_index.write();
        idx.insert_test_symbol(file_id1, SymbolInfo {
            name: "calculate".into(),
            kind: SymbolKind::Function,
            definition: DefinitionLocation { file_id: file_id1, span: make_span(file_id1, 0, 9) },
            type_signature: None,
            is_pub: true,
            documentation: None,
        });
        idx.insert_test_symbol(file_id2, SymbolInfo {
            name: "calculator".into(),
            kind: SymbolKind::Function,
            definition: DefinitionLocation { file_id: file_id2, span: make_span(file_id2, 0, 10) },
            type_signature: None,
            is_pub: true,
            documentation: None,
        });
    }
    db
}

#[test]
fn workspace_symbols_fuzzy_search() {
    let db = setup_test_db();
    let params = WorkspaceSymbolParams {
        query: "calc".to_string(),
        work_done_progress_params: WorkDoneProgressParams { work_done_token: None },
        partial_result_params: PartialResultParams { partial_result_token: None },
    };
    let results = workspace_symbols(&db, &params).expect("should return symbols");
    assert!(results.len() >= 2);
    let names: Vec<&str> = results.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"calculate"));
    assert!(names.contains(&"calculator"));
}
