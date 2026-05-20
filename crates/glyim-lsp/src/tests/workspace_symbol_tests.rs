use crate::navigation::workspace_symbols;
use crate::{AnalysisDatabase, DefinitionLocation, SymbolInfo, SymbolKind};
use glyim_span::{ByteIdx, Span, SyntaxContext};
use lsp_types::*;
use std::path::PathBuf;

fn get_test_path(filename: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(filename);
    path
}

#[test]
#[ignore]
fn workspace_symbols_fuzzy_search() {
    let analysis = AnalysisDatabase::new();
    let path = get_test_path("test.g");
    let file_id = {
        let mut file_map = analysis.file_map.write();
        file_map.get_or_create(&path)
    };
    let span = Span::new(
        file_id,
        ByteIdx::ZERO,
        ByteIdx::from_raw(5),
        SyntaxContext::ROOT,
    );
    let names = vec!["apple", "application", "banana", "ape", "grape"];
    for name in names {
        let sym = SymbolInfo {
            name: name.to_string(),
            kind: SymbolKind::Function,
            definition: DefinitionLocation { file_id, span },
            type_signature: None,
            is_pub: true,
            documentation: None,
        };
        analysis
            .symbol_index
            .write()
            .insert_test_symbol(file_id, sym);
    }

    let params = WorkspaceSymbolParams {
        query: "app".to_string(),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let results = workspace_symbols(&analysis, &params).unwrap();
    let result_names: Vec<_> = results.iter().map(|s| s.name.as_str()).collect();
    assert!(result_names.contains(&"apple"));
    assert!(result_names.contains(&"application"));
    assert!(result_names.contains(&"ape"));
    assert!(!result_names.contains(&"banana"));
    assert!(!result_names.contains(&"grape"));
}

#[test]
#[ignore]
fn workspace_symbols_fuzzy_matching_limit() {
    let analysis = AnalysisDatabase::new();
    let path = get_test_path("test.g");
    let file_id = {
        let mut file_map = analysis.file_map.write();
        file_map.get_or_create(&path)
    };
    let span = Span::new(
        file_id,
        ByteIdx::ZERO,
        ByteIdx::from_raw(5),
        SyntaxContext::ROOT,
    );
    let names = vec!["alpha", "beta", "gamma", "delta", "epsilon"];
    for name in names {
        let sym = SymbolInfo {
            name: name.to_string(),
            kind: SymbolKind::Function,
            definition: DefinitionLocation { file_id, span },
            type_signature: None,
            is_pub: true,
            documentation: None,
        };
        analysis
            .symbol_index
            .write()
            .insert_test_symbol(file_id, sym);
    }

    let params = WorkspaceSymbolParams {
        query: "a".to_string(),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let results = workspace_symbols(&analysis, &params).unwrap();
    let result_names: Vec<_> = results.iter().map(|s| s.name.as_str()).collect();
    assert!(result_names.contains(&"alpha"));
    assert!(result_names.contains(&"beta"));
    assert!(result_names.contains(&"gamma"));
    assert!(!result_names.contains(&"delta"));
    assert!(!result_names.contains(&"epsilon"));
}
