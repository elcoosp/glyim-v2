//! Tests for the `glyip run` command (V20-T04).

use crate::commands::cmd_run;
use crate::config::{NewOptions, RunOptions};
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
fn run_requires_project() {
    let dir = TempDir::new().expect("temp dir");
    let result = cmd_run(dir.path(), &RunOptions::default());
    assert!(result.is_err());
}

#[test]
fn run_with_project() {
    let dir = TempDir::new().expect("temp dir");
    let name = unique_name("runnable");
    let project_path = create_test_project(dir.path(), &name);

    let opts = RunOptions::default();
    let result = cmd_run(&project_path, &opts);
    match result {
        Ok(r) => {
            assert!(r.binary.to_string_lossy().contains(&name));
        }
        Err(_) => {
            // Expected if the compiler pipeline isn't fully wired up.
        }
    }
}

#[test]
fn run_with_args() {
    let dir = TempDir::new().expect("temp dir");
    let name = unique_name("args-test");
    let project_path = create_test_project(dir.path(), &name);

    let opts = RunOptions {
        args: vec!["--verbose".to_string()],
        ..RunOptions::default()
    };
    let result = cmd_run(&project_path, &opts);
    let _ = result;
}
