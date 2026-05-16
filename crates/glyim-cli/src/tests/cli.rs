use crate::{CliArgs, run_with_args};
use clap::Parser;
use std::io::Write;
use tempfile::NamedTempFile;

/// S20-T01: Compile valid file → exit 0
#[test]
fn test_compile_valid_file() {
    let mut tmp = NamedTempFile::new().unwrap();
    write!(tmp, "fn main() {{}}\n").unwrap();
    let path = tmp.into_temp_path();
    let args = CliArgs {
        input: path.to_path_buf(),
        output: None,
        opt_level: 0,
        target: None,
        backend: "llvm".to_string(),
        emit: "obj".to_string(),
    };
    let result = run_with_args(args);
    assert!(
        result.is_ok(),
        "Expected compilation to succeed, got: {:?}",
        result
    );
}

/// S20-T02: Compile invalid file → exit 1
#[test]
fn test_compile_invalid_file() {
    let mut tmp = NamedTempFile::new().unwrap();
    write!(tmp, "fn main() {{").unwrap();
    let path = tmp.into_temp_path();
    let args = CliArgs {
        input: path.to_path_buf(),
        output: None,
        opt_level: 0,
        target: None,
        backend: "llvm".to_string(),
        emit: "obj".to_string(),
    };
    let result = run_with_args(args);
    assert!(result.is_err(), "Expected compilation to fail");
}

/// S20-T03: --help
#[test]
fn test_help_flag() {
    let result = CliArgs::try_parse_from(&["glyim", "--help"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Usage:") || msg.contains("glyim"));
}

/// S20-T04: Missing input → error
#[test]
fn test_missing_input() {
    let result = CliArgs::try_parse_from(&["glyim"]);
    assert!(
        result.is_err(),
        "Expected error for missing required argument"
    );
}

/// S20-T05: --backend bytecode
#[test]
fn test_backend_bytecode_flag() {
    let args = CliArgs::try_parse_from(&["glyim", "--backend", "bytecode", "input.g"]).unwrap();
    assert_eq!(args.backend, "bytecode");
}

/// S20-T06: --emit mir flag
#[test]
fn test_emit_mir_flag() {
    let args = CliArgs::try_parse_from(&["glyim", "--emit", "mir", "input.g"]).unwrap();
    assert_eq!(args.emit, "mir");
}

/// S20-T07: --emit llvm-ir flag
#[test]
fn test_emit_llvm_ir_flag() {
    let args = CliArgs::try_parse_from(&["glyim", "--emit", "llvm-ir", "input.g"]).unwrap();
    assert_eq!(args.emit, "llvm-ir");
}
