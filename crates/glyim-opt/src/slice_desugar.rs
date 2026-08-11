//! MIR-level desugaring of slice projections.
//! Currently a stub; full implementation will be added later.

use glyim_mir::Body;
use glyim_type::TyCtx;

/// Desugar `ProjectionElem::Slice` into explicit MIR statements.
/// Currently a no-op stub.
pub fn run(_ctx: &TyCtx, _body: &mut Body) {
    // FIXME: implement actual desugaring
}
