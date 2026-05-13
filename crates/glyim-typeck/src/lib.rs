pub mod thir;
use glyim_type::*;
pub struct TypeckResult;
pub fn typeck_crate(_ctx: TyCtxMut, _def_map: &glyim_def_map::CrateDefMap, _hir: &glyim_hir::CrateHir, _solver: &mut dyn glyim_solve::TraitSolver) -> (TyCtx, TypeckResult) {
    (TyCtx, TypeckResult)
}
