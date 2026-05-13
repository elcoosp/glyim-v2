use glyim_mir::Body as MirBody;
use glyim_typeck::thir::Body as ThirBody;
use glyim_diag::GlyimDiagnostic;

pub struct LowerResult {
    pub body: MirBody,
    pub diagnostics: Vec<GlyimDiagnostic>,
}

pub trait LowerCtx {}

pub fn lower_body(_ctx: &dyn LowerCtx, _thir: &ThirBody) -> LowerResult {
    // STUB: create a dummy MIR body
    use glyim_core::def_id::{DefId, CrateId, LocalDefId};
    let dummy_def_id = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0));
    let body = MirBody::dummy(dummy_def_id);
    LowerResult { body, diagnostics: Vec::new() }
}
