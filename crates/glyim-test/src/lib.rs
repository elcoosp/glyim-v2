pub mod error;
pub mod harness;
pub mod annotations;
pub mod comparison;
pub mod mock;
pub mod assertions;
pub mod snapshot;
pub mod phase;
pub mod property;
pub mod fixtures;

pub use error::{TestDiscoveryError, FailureReason, TimeoutError, AssertionFailure};

pub use harness::{TestRunner, TestPlan, TestMode, runner::{ProgramRunner, RunResult, OutputCheck}};
pub use mock::{MockSolver, MockCodegen, MockBorrowckCtx, MockLowerCtx, TestDbBuilder};
pub use assertions::{
    assert_ty, TyAssert, check_ty, TyCheck,
    assert_mir, MirAssert,
    assert_no_errors, assert_has_errors, assert_error_count,
    assert_diag_contains, assert_diag_code, assert_has_severity,
    assert_layout,
};
pub use snapshot::{snapshot_cst, snapshot_mir, snapshot_def_map};
pub use phase::{FrontendTester, AnalysisTester, MirGenTester, CodegenTester, CompilationTrace};
pub use fixtures::{SourceBuilder, TyCtxBuilder, TyFactory};
pub use property::check_ty_property;

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
