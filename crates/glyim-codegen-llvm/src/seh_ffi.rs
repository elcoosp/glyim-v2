//! Raw FFI bindings for the LLVM-C exception-handling (funclet) API.
//!
//! These functions are part of LLVM-C's stable `llvm-c/Core.h` (present since
//! LLVM 8), but the pinned `inkwell` 0.10 / `llvm-sys` 221.0.1 combination does
//! not *bind* them. We declare them directly against `llvm-sys`'s raw pointer
//! types so they resolve against the exact same linked `libLLVM` instance that
//! `inkwell` uses (do not link a second copy of LLVM).
//!
//! These are only needed for the `Personality::Seh` lowering path
//! (`emit_seh_cleanuppad` / `emit_seh_cleanupret`). See `lower.rs`.

use llvm_sys::prelude::{LLVMBasicBlockRef, LLVMBuilderRef, LLVMValueRef};
use std::os::raw::{c_char, c_uint};

unsafe extern "C" {
    pub fn LLVMBuildCleanupPad(
        B: LLVMBuilderRef,
        ParentPad: LLVMValueRef,
        Args: *mut LLVMValueRef,
        NumArgs: c_uint,
        Name: *const c_char,
    ) -> LLVMValueRef;

    pub fn LLVMBuildCleanupRet(
        B: LLVMBuilderRef,
        CleanupPad: LLVMValueRef,
        BB: LLVMBasicBlockRef,
    ) -> LLVMValueRef;

    pub fn LLVMBuildCatchSwitch(
        B: LLVMBuilderRef,
        ParentPad: LLVMValueRef,
        UnwindBB: LLVMBasicBlockRef,
        NumHandlers: c_uint,
        Name: *const c_char,
    ) -> LLVMValueRef;

    pub fn LLVMBuildCatchPad(
        B: LLVMBuilderRef,
        ParentPad: LLVMValueRef,
        Args: *mut LLVMValueRef,
        NumArgs: c_uint,
        Name: *const c_char,
    ) -> LLVMValueRef;

    pub fn LLVMBuildCatchRet(
        B: LLVMBuilderRef,
        CatchPad: LLVMValueRef,
        BB: LLVMBasicBlockRef,
    ) -> LLVMValueRef;

    pub fn LLVMAddHandler(CatchSwitch: LLVMValueRef, Dest: LLVMBasicBlockRef);
}

/// Helper: a null-terminated C string for an instruction name, used by the
/// raw `LLVMBuild*` funclet builders which take a `const char *Name`.
///
/// We keep the temporary in a `CString` at the call site; this helper just
/// hands back the raw pointer. Marked `#[allow(dead_code)]` until the catch-pad
/// paths (currently unused — only cleanuppad/cleanupret are wired) need it.
#[allow(dead_code)]
pub(crate) fn c_name(name: &str) -> std::ffi::CString {
    std::ffi::CString::new(name).unwrap_or_else(|_| std::ffi::CString::new("").unwrap())
}
