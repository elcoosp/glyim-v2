use crate::linker::{invoke_linker, linker_flags_for_target};
use std::path::PathBuf;

#[test]
fn test_invoke_linker_basic() {
    let obj = PathBuf::from("dummy.o");
    let out = PathBuf::from("dummy_out");
    let result = invoke_linker(&obj, &out, None, None, None);
    let _ = result;
}

// Plan §18.1: cross-compile target → linker-flag table.
#[test]
fn test_linker_flags_for_known_target() {
    // aarch64 host-vs-target must produce --target + GNU ld -m emulation.
    let flags = linker_flags_for_target("aarch64-unknown-linux-gnu").unwrap();
    assert_eq!(flags, vec!["--target", "aarch64-unknown-linux-gnu", "-m", "aarch64linux"]);
}

#[test]
fn test_linker_flags_for_x86_64() {
    let flags = linker_flags_for_target("x86_64-unknown-linux-gnu").unwrap();
    assert_eq!(flags, vec!["--target", "x86_64-unknown-linux-gnu", "-m", "elf_x86_64"]);
}

#[test]
fn test_linker_flags_for_unknown_target_errors() {
    // An unmapped target must be a hard error rather than silently passing
    // host flags to a cross target.
    let err = linker_flags_for_target("sparc64-unknown-linux-gnu");
    assert!(err.is_err());
    let msg = err.unwrap_err();
    assert!(msg.contains("sparc64-unknown-linux-gnu"));
    assert!(msg.contains("not in the supported linker-flag table"));
}
