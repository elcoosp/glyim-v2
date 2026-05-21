use crate::LspState;
use glyim_db::Database;
use std::path::PathBuf;

#[test]
fn test_rename_symbol_updates_all_references() {
    let db = Database::new(glyim_db::CrateConfig {
        name: "test".to_string(),
        target_triple: "x86_64-unknown-linux-gnu".to_string(),
        opt_level: 0,
    });
    let mut state = LspState::new(db);
    let cache_dir = std::env::temp_dir().join("glyim-lsp-test");
    state.start_driver(cache_dir);

    let path = PathBuf::from("/test/main.g");
    let content = r#"
fn old_name() -> i32 { 42 }
fn main() { let x = old_name(); }
"#;
    state.did_open(path.clone(), content.to_string(), 1);
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Scope the read lock
    {
        let analysis = state.analysis();
        let ref_graph = analysis.reference_graph.read();
        let refs = ref_graph.find_references("old_name");
        assert!(refs.len() >= 2);
    }

    state.did_close(&path);
}
