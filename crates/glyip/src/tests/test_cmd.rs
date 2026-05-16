//! Tests for the `glyip test` command (V20-T03).

use crate::commands::cmd_test;
use crate::config::{NewOptions, TestOptions};
use std::sync::atomic::{AtomicU64, Ordering};
use tempfile::TempDir;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_name(base: &str) -> String {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}", base, id)
}

fn create_test_project(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    std::env::set_current_dir(dir).expect("cd");
    let result = crate::commands::cmd_new(name, &NewOptions::default()).expect("new");
    result.path
}

#[test]
fn test_with_only_src_files() {
    let dir = TempDir::new().expect("temp dir");
    let name = unique_name("no-tests");
    let project_path = create_test_project(dir.path(), &name);

    // Remove the tests/ directory entirely.
    let test_dir = project_path.join("tests");
    if test_dir.exists() {
        std::fs::remove_dir_all(&test_dir).expect("remove tests dir");
    }

    let opts = TestOptions {
        no_run: true,
        ..TestOptions::default()
    };
    let result = cmd_test(&project_path, &opts).expect("test");
    // src/main.g still exists, so total >= 1
    assert!(result.total >= 1);
}

#[test]
fn test_with_filter() {
    let dir = TempDir::new().expect("temp dir");
    let name = unique_name("filtered");
    let project_path = create_test_project(dir.path(), &name);

    std::fs::write(
        project_path.join("tests/match_this.g"),
        "// test that matches filter",
    )
    .expect("write test");

    let opts = TestOptions {
        filter: Some("match_this".to_string()),
        no_run: true,
        ..TestOptions::default()
    };
    let result = cmd_test(&project_path, &opts).expect("test");
    // At least the filtered test file should be counted.
    assert!(result.total >= 1);
}

#[test]
fn test_no_run_flag() {
    let dir = TempDir::new().expect("temp dir");
    let name = unique_name("no-run");
    let project_path = create_test_project(dir.path(), &name);

    let opts = TestOptions {
        no_run: true,
        ..TestOptions::default()
    };
    let result = cmd_test(&project_path, &opts);
    assert!(result.is_ok());
}
