use crate::state::LspState;
use glyim_test::mock::TestDbBuilder;
use std::path::PathBuf;

#[test]
fn did_open_registers_file() {
    let db = TestDbBuilder::new()
        .name("test")
        .target_triple("x86_64-unknown-linux-gnu")
        .opt_level(0)
        .build();
    let mut state = LspState::new(db);
    let path = PathBuf::from("/tmp/test.g");
    let content = "fn main() {}".to_string();

    state.did_open(path.clone(), content.clone(), 1);

    // After did_open, file_content should return the same content.
    let stored = state.file_content(&path);
    assert_eq!(stored, Some(content));
}

#[test]
fn did_open_with_content_returns_same() {
    let db = TestDbBuilder::new()
        .name("test2")
        .target_triple("x86_64-unknown-linux-gnu")
        .opt_level(0)
        .build();
    let mut state = LspState::new(db);
    let path = PathBuf::from("/tmp/bar.g");
    let content = "let x = 42;".to_string();

    state.did_open(path.clone(), content.clone(), 2);
    let stored = state.file_content(&path);
    assert_eq!(stored, Some(content));
}

#[test]
fn did_close_removes_file() {
    let db = TestDbBuilder::new()
        .name("test3")
        .target_triple("x86_64-unknown-linux-gnu")
        .opt_level(0)
        .build();
    let mut state = LspState::new(db);
    let path = PathBuf::from("/tmp/close.g");
    state.did_open(path.clone(), "stuff".to_string(), 1);
    state.did_close(&path);
    assert!(state.file_content(&path).is_none());
}
