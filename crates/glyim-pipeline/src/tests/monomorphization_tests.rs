use glyim_core::def_id::{CrateId, DefId, LocalDefId};
use glyim_core::primitives::IntTy;
use glyim_mir::Body;
use glyim_test::with_fresh_ty_ctx;
use glyim_type::{GenericArg, ParamTy, Substitution, TyKind};

fn dummy_def_id() -> DefId {
    DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0))
}

#[test]
fn substitute_body_replaces_param_with_concrete() {
    let (ty_ctx, _) = with_fresh_ty_ctx(|ctx_mut| {
        let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
        let substs = ctx_mut.intern_substitution(vec![GenericArg::Ty(i32_ty)]);
        let mut body = Body::dummy(dummy_def_id());
        let param_ty = TyKind::Param(ParamTy { index: 0, name: "T".into() });
        let param_ty = ctx_mut.mk_ty(param_ty);
        body.return_ty = param_ty;
        let ty_ctx = ctx_mut.freeze();
        let substituted = crate::mono_cache::substitute_body(&body, &substs, &ty_ctx);
        match ty_ctx.ty_kind(substituted.return_ty) {
            TyKind::Int(IntTy::I32) => (),
            _ => panic!("Substitution failed"),
        }
    });
}

#[test]
fn substitute_body_empty_substitution_leaves_param() {
    let (ty_ctx, _) = with_fresh_ty_ctx(|ctx_mut| {
        let substs = Substitution::empty();
        let mut body = Body::dummy(dummy_def_id());
        let param_ty = TyKind::Param(ParamTy { index: 0, name: "T".into() });
        let param_ty = ctx_mut.mk_ty(param_ty);
        body.return_ty = param_ty;
        let ty_ctx = ctx_mut.freeze();
        let substituted = crate::mono_cache::substitute_body(&body, &substs, &ty_ctx);
        match ty_ctx.ty_kind(substituted.return_ty) {
            TyKind::Param(_) => (),
            _ => panic!("Should remain ParamTy"),
        }
    });
}
