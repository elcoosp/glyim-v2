use crate::LspState;
use glyim_db::Database;
use std::path::PathBuf;

#[test]
fn test_function_signature_change_updates_symbol_index() {
    let db = Database::new(glyim_db::CrateConfig {
        name: "test".to_string(),
        target_triple: "x86_64-unknown-linux-gnu".to_string(),
        opt_level: 0,
    });
    let mut state = LspState::new(db);
    let cache_dir = std::env::temp_dir().join("glyim-lsp-test");
    state.start_driver(cache_dir);

    let path = PathBuf::from("/test/main.g");
    let initial_content = r#"
fn add(a: i32, b: i32) -> i32 { a + b }
"#;
    state.did_open(path.clone(), initial_content.to_string(), 1);
    std::thread::sleep(std::time::Duration::from_millis(100));

    // First verification: scope the read lock
    {
        let analysis = state.analysis();
        let file_id = state.file_id(&path).unwrap();
        let symbol_index = analysis.symbol_index.read();
        let symbols = symbol_index.lookup_by_name("add");
        assert_eq!(symbols.len(), 1);
        let sym = symbols[0];
        let sig = sym.type_signature.as_ref().unwrap();
        assert_eq!(sig.params.len(), 2);
        assert_eq!(sig.params[0].0, "a");
        assert!(sig.params[0].1.contains("i32"));
        assert_eq!(sig.return_type.as_ref().unwrap(), "i32");
    } // read lock released here

    let updated_content = r#"
fn add(a: i64, b: i64) -> i64 { a + b }
"#;
    state.did_change(path.clone(), updated_content.to_string(), 2);
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Second verification
    {
        let analysis = state.analysis();
        let symbol_index2 = analysis.symbol_index.read();
        let symbols2 = symbol_index2.lookup_by_name("add");
        assert_eq!(symbols2.len(), 1);
        let sym2 = symbols2[0];
        let sig2 = sym2.type_signature.as_ref().unwrap();
        assert_eq!(sig2.params[0].1, "i64");
        assert_eq!(sig2.return_type.as_ref().unwrap(), "i64");
    }

    state.did_close(&path);
}

#[test]
fn test_changing_file_recompiles_only_affected_files() {
    // Placeholder - will be implemented after dependency graph
    let _ = ();
}
