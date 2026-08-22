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
    // When invoked through the `cc`/clang *driver*, GNU-ld-specific flags
    // (`-m <emulation>`) are invalid; the driver wants a single-dash
    // `-target`. The raw-GNU-ld path keeps `-m` and is covered separately.
    let flags = linker_flags_for_target("aarch64-unknown-linux-gnu", "cc").unwrap();
    assert_eq!(flags, vec!["-target", "aarch64-unknown-linux-gnu"]);
}

#[test]
fn test_linker_flags_for_x86_64() {
    let flags = linker_flags_for_target("x86_64-unknown-linux-gnu", "cc").unwrap();
    assert_eq!(flags, vec!["-target", "x86_64-unknown-linux-gnu"]);
}

#[test]
fn test_linker_flags_raw_ld_keeps_emulation() {
    // A raw GNU `ld` has no `--target`; it needs only the `-m <emulation>` flag.
    let flags = linker_flags_for_target("x86_64-unknown-linux-gnu", "ld").unwrap();
    assert_eq!(flags, vec!["-m", "elf_x86_64"]);
}

#[test]
fn test_linker_flags_for_unknown_target_errors() {
    // An unmapped target must be a hard error rather than silently passing
    // host flags to a cross target. Only the raw-ld path validates the triple
    // (drivers like `cc` accept any triple and let the toolchain reject later).
    let err = linker_flags_for_target("sparc64-unknown-linux-gnu", "ld");
    assert!(err.is_err());
    let msg = err.unwrap_err();
    assert!(msg.contains("sparc64-unknown-linux-gnu"));
    assert!(msg.contains("not in the supported linker-flag table"));
}
