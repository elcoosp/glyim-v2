#![allow(missing_docs)]
// Stylistic clippy lints suppressed crate-wide (test-noise lints).
#![allow(
    clippy::cloned_ref_to_slice_refs,
    clippy::vec_init_then_push,
    clippy::assertions_on_constants,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::manual_c_str_literals,
    clippy::doc_lazy_continuation,
    clippy::empty_line_after_doc_comments,
    clippy::manual_strip,
    clippy::needless_range_loop,
    clippy::unnecessary_cast,
    clippy::clone_on_copy,
    clippy::mutable_key_type,
    clippy::only_used_in_recursion,
    clippy::let_unit_value,
    clippy::unnecessary_literal_unwrap,
    clippy::format_in_format_args,
    clippy::permissions_set_readonly_false,
    clippy::needless_lifetimes,
    clippy::collapsible_if
)]
pub mod annotations;
pub mod assertions;
pub mod comparison;
pub mod error;
pub mod fixtures;
pub mod harness;
pub mod mock;
pub mod phase;
pub mod property;
pub mod snapshot;

pub use error::{AssertionFailure, FailureReason, TestDiscoveryError, TimeoutError};

pub use assertions::{
    MirAssert, TyAssert, TyCheck, assert_diag_code, assert_diag_contains, assert_error_count,
    assert_has_errors, assert_has_severity, assert_layout, assert_mir, assert_no_errors, assert_ty,
    check_ty,
};
pub use fixtures::{SourceBuilder, TyCtxBuilder, TyFactory};
pub use harness::{
    TestMode, TestPlan, TestRunner,
    runner::{OutputCheck, ProgramRunner, RunResult},
};
pub use mock::{MockBorrowckCtx, MockCodegen, MockLowerCtx, MockSolver, TestDbBuilder};
pub use phase::{AnalysisTester, CodegenTester, CompilationTrace, FrontendTester, MirGenTester};
pub use property::check_ty_property;
pub use snapshot::{snapshot_cst, snapshot_def_map, snapshot_mir};

use glyim_type::{TyCtx, TyCtxMut};

#[cfg(test)]
mod tests;

pub fn test_ty_ctx() -> TyCtxMut {
    TyCtxBuilder::new().build_mut()
}

pub fn test_frozen_ty_ctx() -> TyCtx {
    test_ty_ctx().freeze()
}

pub fn with_fresh_ty_ctx<F, R>(f: F) -> (TyCtx, R)
where
    F: FnOnce(&mut TyCtxMut) -> R,
{
    let mut ctx_mut = test_ty_ctx();
    let result = f(&mut ctx_mut);
    (ctx_mut.freeze(), result)
}
