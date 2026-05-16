use crate::state::LspState;
use glyim_db::{Database, CrateConfig};
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn diagnostics_are_emitted_on_change() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let config = CrateConfig {
            name: "test".into(),
            target_triple: "x86_64".into(),
            opt_level: 0,
        };
        let db = Database::new(config);
        let mut state = LspState::new(db);
        let cache_dir = std::env::temp_dir().join("glyim-lsp-test");
        state.start_driver(cache_dir);
        let path = PathBuf::from("/test/main.g");
        let content = "fn main() { let x = 42; }".to_string();
        state.did_open(path.clone(), content, 1);
        tokio::time::sleep(Duration::from_millis(100)).await;
        let diags = state.diagnostics_for_file(&path);
        assert!(diags.is_empty(), "Initial file should have no diagnostics");
        // Use content that contains the substring "error" to trigger a diagnostic
        let invalid = "fn main() { error; }".to_string();
        state.did_change(path.clone(), invalid, 2);
        tokio::time::sleep(Duration::from_millis(100)).await;
        let diags2 = state.diagnostics_for_file(&path);
        assert!(!diags2.is_empty(), "File with 'error' should produce diagnostics");
    });
}
