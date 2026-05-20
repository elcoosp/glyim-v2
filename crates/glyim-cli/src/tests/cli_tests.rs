use clap::Parser;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

// Helper to parse args from strings
fn parse_args(args: &[&str]) -> crate::CliArgs {
    crate::CliArgs::parse_from(args)
}

#[test]
fn t01_cli_parses_target_and_opt_level() {
    let args = parse_args(&["glyim", "input.g", "--target", "aarch64-unknown-linux-gnu", "-O", "2"]);
    assert_eq!(args.input, PathBuf::from("input.g"));
    assert_eq!(args.target, Some("aarch64-unknown-linux-gnu".to_string()));
    assert_eq!(args.opt_level, 2);
    assert_eq!(args.emit, "obj");
    assert_eq!(args.backend, "llvm");

    // Test that CrateConfig is built correctly
    let config = glyim_db::CrateConfig {
        name: "input".to_string(),
        target_triple: args.target.clone().unwrap(),
        opt_level: args.opt_level,
    };
    assert_eq!(config.target_triple, "aarch64-unknown-linux-gnu");
    assert_eq!(config.opt_level, 2);
}

#[test]
fn t01_default_target_and_opt_level() {
    let args = parse_args(&["glyim", "input.g"]);
    assert_eq!(args.target, None);
    assert_eq!(args.opt_level, 0);
    // Default target triple should be x86_64-unknown-linux-gnu
    let config = glyim_db::CrateConfig {
        name: "input".to_string(),
        target_triple: args.target.unwrap_or_else(|| "x86_64-unknown-linux-gnu".to_string()),
        opt_level: args.opt_level,
    };
    assert_eq!(config.target_triple, "x86_64-unknown-linux-gnu");
}

#[test]
fn t02_pipeline_produces_output_binary() {
    let dir = tempdir().expect("failed to create temp dir");
    let source_path = dir.path().join("test.g");
    let output_path = dir.path().join("test.o");
    fs::write(&source_path, "fn main() {}\n").expect("failed to write source");

    let args = crate::CliArgs {
        input: source_path.clone(),
        output: Some(output_path.clone()),
        emit: "obj".to_string(),
        opt_level: 0,
        target: None,
        backend: "llvm".to_string(), // use LLVM backend which actually writes an object file
    };

    let result = crate::run_with_args(args);
    assert!(result.is_ok(), "compilation failed: {:?}", result);
    assert!(output_path.exists(), "output file not created");
    let metadata = fs::metadata(&output_path).expect("failed to get metadata");
    assert!(metadata.len() > 0, "output file is empty");
}

#[test]
fn t03_cli_reports_compilation_errors() {
    let dir = tempdir().expect("failed to create temp dir");
    let source_path = dir.path().join("bad.g");
    let output_path = dir.path().join("bad.o");
    // Write source with a syntax error: missing semicolon
    fs::write(&source_path, "fn main() { let x = 42 }\n").expect("failed to write source");

    let args = crate::CliArgs {
        input: source_path,
        output: Some(output_path),
        emit: "obj".to_string(),
        opt_level: 0,
        target: None,
        backend: "bytecode".to_string(),
    };

    let result = crate::run_with_args(args);
    assert!(result.is_err(), "compilation unexpectedly succeeded");
    let errs = result.unwrap_err();
    assert!(!errs.is_empty(), "expected at least one diagnostic");
    // Collect all error messages as strings
    let all_messages: String = errs.iter().map(|d| d.message.as_str()).collect::<Vec<_>>().join(" ");
    assert!(
        all_messages.contains("semicolon") || all_messages.contains("expected") || all_messages.contains("error"),
        "error message missing expected content: {}",
        all_messages
    );
}
