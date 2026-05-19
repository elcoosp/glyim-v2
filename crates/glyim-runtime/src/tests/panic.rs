//! Tests for glyim_panic FFI function.
//!
//! S06-T03: glyim_panic aborts execution.
//!
//! Since glyim_panic calls std::process::abort() which terminates the
//! process immediately, we test it by spawning a child process that
//! calls glyim_panic and verifying the child is terminated abnormally.

use crate::glyim_panic;

/// Test that glyim_panic aborts the process.
///
/// This test spawns a child process that calls glyim_panic and verifies
/// that the child is terminated with SIGABRT (on Unix) or a non-zero
/// exit code (on other platforms).
#[test]
fn panic_aborts_process() {
    // If we're the child process, trigger the panic
    if std::env::var("__GLYIM_PANIC_CHILD").is_ok() {
        // SAFETY: We're in a test subprocess that's expected to abort.
        // The null pointer is acceptable because glyim_panic ignores
        // the message parameters in the current implementation.
        glyim_panic(std::ptr::null(), 0);
    }

    // Parent process: spawn child and verify it aborts
    let exe = std::env::current_exe().unwrap_or_else(|e| {
        panic!("failed to get current executable: {}", e);
    });

    let output = std::process::Command::new(&exe)
        .env("__GLYIM_PANIC_CHILD", "1")
        .output()
        .unwrap_or_else(|e| {
            panic!("failed to spawn child process: {}", e);
        });

    // The process must not have exited successfully
    assert!(
        !output.status.success(),
        "glyim_panic should cause process abort, but child exited successfully"
    );

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        let signal = output.status.signal();
        assert_eq!(
            signal,
            Some(6),
            "expected SIGABRT (signal 6) from abort(), got signal {:?}",
            signal
        );
    }

    #[cfg(not(unix))]
    {
        let code = output.status.code();
        assert!(
            code.is_some() && code != Some(0),
            "expected non-zero exit code from abort(), got {:?}",
            code
        );
    }
}
