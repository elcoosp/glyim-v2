//! Tests for the `glyip test` command — S12-T04.

use crate::commands::cmd_test;
use crate::config::TestOptions;
use std::fs;
use tempfile::TempDir;

#[test]
fn cmd_test_no_tests_no_run() {
    // S12-T04: glyip test with no test files and no_run should succeed.
    let dir = TempDir::new().unwrap();
    let pd = dir.path().join("empty-proj");
    fs::create_dir_all(pd.join("src")).unwrap();
    fs::write(
        pd.join("Glyip.toml"),
        "[package]\nname = \"empty-proj\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(pd.join("src/main.g"), "fn main() {}\n").unwrap();

    let opts = TestOptions {
        release: false,
        filter: None,
        no_run: true,
    };

    let result = cmd_test(&pd, &opts).unwrap();
    // cmd_test scans both tests/ and src/ for .g files.
    // With only src/main.g and no tests/ directory, total counts src/main.g.
    // The key assertion is that no tests actually fail (failed == 0).
    assert_eq!(result.failed, 0, "no test failures expected");
    assert_eq!(result.passed, 0, "no test passes expected without running");
}

#[test]
fn cmd_test_with_filter_skips_non_matching() {
    let dir = TempDir::new().unwrap();
    let pd = dir.path().join("filtered-proj");
    fs::create_dir_all(pd.join("src")).unwrap();
    fs::create_dir_all(pd.join("tests")).unwrap();
    fs::write(
        pd.join("Glyip.toml"),
        "[package]\nname = \"filtered-proj\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(pd.join("src/main.g"), "fn main() {}\n").unwrap();
    fs::write(
        pd.join("tests/math_test.g"),
        "// math tests\nfn test_add() {}\n",
    )
    .unwrap();
    fs::write(
        pd.join("tests/string_test.g"),
        "// string tests\nfn test_len() {}\n",
    )
    .unwrap();

    let opts = TestOptions {
        release: false,
        filter: Some("math".to_string()),
        no_run: true,
    };

    let result = cmd_test(&pd, &opts).unwrap();
    assert!(
        result.ignored >= 1,
        "non-matching test files should be ignored, got ignored={}",
        result.ignored
    );
}

#[test]
fn cmd_test_no_run_counts_test_files() {
    let dir = TempDir::new().unwrap();
    let pd = dir.path().join("counted-proj");
    fs::create_dir_all(pd.join("src")).unwrap();
    fs::create_dir_all(pd.join("tests")).unwrap();
    fs::write(
        pd.join("Glyip.toml"),
        "[package]\nname = \"counted-proj\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(pd.join("src/main.g"), "fn main() {}\n").unwrap();
    fs::write(pd.join("tests/a_test.g"), "// test a\nfn test_a() {}\n").unwrap();
    fs::write(pd.join("tests/b_test.g"), "// test b\nfn test_b() {}\n").unwrap();

    let opts = TestOptions {
        release: false,
        filter: None,
        no_run: true,
    };

    let result = cmd_test(&pd, &opts).unwrap();
    assert!(
        result.total >= 2,
        "should count at least the 2 test files, got total={}",
        result.total
    );
}

#[test]
fn test_result_fields() {
    let result = crate::commands::TestResult {
        total: 10,
        passed: 7,
        failed: 2,
        ignored: 1,
    };
    assert_eq!(result.total, 10);
    assert_eq!(result.passed, 7);
    assert_eq!(result.failed, 2);
    assert_eq!(result.ignored, 1);
}
