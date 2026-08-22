/// diag.
pub mod diag;
/// layout.
pub mod layout;
/// mir.
pub mod mir;
/// span.
pub mod span;
/// ty.
pub mod ty;

pub use diag::{
    assert_diag_code, assert_diag_contains, assert_error_count, assert_has_errors,
    assert_has_severity, assert_no_errors,
};
pub use layout::assert_layout;
pub use mir::{MirAssert, assert_mir};
pub use ty::{TyAssert, TyCheck, assert_ty, assert_ty_eq, check_ty};
