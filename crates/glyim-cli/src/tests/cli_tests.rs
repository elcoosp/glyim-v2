use crate::{CliArgs, run_with_args};
use clap::Parser;
use std::io::Write;
use tempfile::NamedTempFile;

/// S20-T01: Compile valid file → exit 0
#[test]
fn test_compile_valid_file() {
    let mut tmp = NamedTempFile::new().unwrap();
    writeln!(tmp, "fn main() {{}}").unwrap();
    let path = tmp.into_temp_path();
    let args = CliArgs {
        input: path.to_path_buf(),
        output: None,
        opt_level: 0,
        target: None,
        backend: "llvm".to_string(),
        emit: "obj".to_string(),
        linker: None,
        link_flags: None,
        lto: "off".to_string(),
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
        linker: None,
        link_flags: None,
        lto: "off".to_string(),
    };
    let result = run_with_args(args);
    assert!(result.is_err(), "Expected compilation to fail");
}

/// S20-T03: --help
#[test]
fn test_help_flag() {
    let result = CliArgs::try_parse_from(["glyim", "--help"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Usage:") || msg.contains("glyim"));
}

/// S20-T04: Missing input → error
#[test]
fn test_missing_input() {
    let result = CliArgs::try_parse_from(["glyim"]);
    assert!(
        result.is_err(),
        "Expected error for missing required argument"
    );
}

/// S20-T05: --backend bytecode
#[test]
fn test_backend_bytecode_flag() {
    let args = CliArgs::try_parse_from(["glyim", "--backend", "bytecode", "input.g"]).unwrap();
    assert_eq!(args.backend, "bytecode");
}

/// S20-T06: --emit mir flag
#[test]
fn test_emit_mir_flag() {
    let args = CliArgs::try_parse_from(["glyim", "--emit", "mir", "input.g"]).unwrap();
    assert_eq!(args.emit, "mir");
}

/// S20-T07: --emit llvm-ir flag
#[test]
fn test_emit_llvm_ir_flag() {
    let args = CliArgs::try_parse_from(["glyim", "--emit", "llvm-ir", "input.g"]).unwrap();
    assert_eq!(args.emit, "llvm-ir");
}

/// §18.3: --emit=asm produces a `.s` assembly file containing recognizable
/// host assembly (a function epilogue `ret` instruction at minimum).
#[test]
fn test_emit_asm_produces_assembly_file() {
    let mut tmp = NamedTempFile::new().unwrap();
    writeln!(tmp, "fn main() {{}}").unwrap();
    let src = tmp.into_temp_path();

    let out_dir = tempfile::tempdir().unwrap();
    let asm_path = out_dir.path().join("out.s");

    let args = CliArgs {
        input: src.to_path_buf(),
        output: Some(asm_path.clone()),
        opt_level: 0,
        target: None,
        backend: "llvm".to_string(),
        emit: "asm".to_string(),
        linker: None,
        link_flags: None,
        lto: "off".to_string(),
    };
    let result = run_with_args(args);
    assert!(result.is_ok(), "asm emit should succeed, got: {:?}", result);

    assert!(asm_path.exists(), "assembly output file should exist");
    let asm = std::fs::read_to_string(&asm_path).unwrap();
    // A trivial `fn main() {}` must lower to at least a `ret` instruction on
    // every supported host backend.
    assert!(
        asm.contains("ret"),
        "assembly output should contain a `ret` instruction; got:\n{}",
        asm
    );
}

/// Phase 10.2: `--lto fat` engages the in-compiler Fat LTO pass
/// (`run_lto`) over the compiled module. With a single entry file there is no
/// second module to merge, so Fat degrades to running the optimization pipeline
/// once (which is exactly the documented single-module behaviour) and the
/// compile must still succeed. opt_level is kept at 0 here so the test
/// exercises the LTO *wiring* (run_lto is invoked with `Fat`) rather than the
/// unrelated O2 codegen path.
#[test]
fn test_lto_fat_compiles_to_object() {
    let mut tmp = NamedTempFile::new().unwrap();
    writeln!(tmp, "fn main() {{}}").unwrap();
    let path = tmp.into_temp_path();
    let out_dir = tempfile::tempdir().unwrap();
    let obj_path = out_dir.path().join("out.o");

    let args = CliArgs {
        input: path.to_path_buf(),
        output: Some(obj_path.clone()),
        opt_level: 0,
        target: None,
        backend: "llvm".to_string(),
        emit: "obj".to_string(),
        linker: None,
        link_flags: None,
        lto: "fat".to_string(),
    };
    let result = run_with_args(args);
    assert!(
        result.is_ok(),
        "LTO=fat compile should succeed (engages run_lto Fat), got: {:?}",
        result
    );
    assert!(obj_path.exists(), "object output should exist for LTO=fat");
}

/// Phase 10.2: `--lto thin` is a tracked gap (linker-driver integration) and
/// must surface an explicit error rather than silently degrading to a no-op.
#[test]
fn test_lto_thin_surfaces_tracked_gap() {
    let mut tmp = NamedTempFile::new().unwrap();
    writeln!(tmp, "fn main() {{}}").unwrap();
    let path = tmp.into_temp_path();

    let args = CliArgs {
        input: path.to_path_buf(),
        output: None,
        opt_level: 2,
        target: None,
        backend: "llvm".to_string(),
        emit: "obj".to_string(),
        linker: None,
        link_flags: None,
        lto: "thin".to_string(),
    };
    let result = run_with_args(args);
    assert!(
        result.is_err(),
        "LTO=thin must surface its tracked-gap error, not silently no-op"
    );
}

/// Phase 10.2: an invalid `--lto` value is rejected with a parse error.
#[test]
fn test_lto_invalid_value_rejected() {
    let mut tmp = NamedTempFile::new().unwrap();
    writeln!(tmp, "fn main() {{}}").unwrap();
    let path = tmp.into_temp_path();

    let args = CliArgs {
        input: path.to_path_buf(),
        output: None,
        opt_level: 0,
        target: None,
        backend: "llvm".to_string(),
        emit: "obj".to_string(),
        linker: None,
        link_flags: None,
        lto: "bogus".to_string(),
    };
    let result = run_with_args(args);
    assert!(result.is_err(), "invalid --lto value must be rejected");
}
