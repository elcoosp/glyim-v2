use glyim_mir::Body;
use glyim_diag::GlyimDiagnostic;
pub struct BorrowckResult { pub errors: Vec<GlyimDiagnostic> }
pub trait BorrowckCtx {}
pub fn check_borrows(_ctx: &dyn BorrowckCtx, _body: &Body) -> BorrowckResult {
    BorrowckResult { errors: Vec::new() }
}
