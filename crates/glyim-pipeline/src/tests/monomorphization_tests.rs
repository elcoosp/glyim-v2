use glyim_core::def_id::{CrateId, DefId, LocalDefId};
use glyim_core::primitives::IntTy;
use glyim_mir::*;
use glyim_span::Span;
use glyim_test::with_fresh_ty_ctx;
use glyim_type::{GenericArg, ParamTy, Substitution, Ty, TyKind};

use crate::mono_cache::substitute_body; // we need to make this visible for tests

fn dummy_def_id() -> DefId {
    DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0))
}

#[test]
fn substitute_body_replaces_generic_param() {
    let (ty_ctx, _) = with_fresh_ty_ctx(|ctx_mut| {
        // Create a substitution: T -> i32
        let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
        let substs = ctx_mut.intern_substitution(vec![GenericArg::Ty(i32_ty)]);

        // Create a dummy MIR body that uses a ParamTy with index 0
        let mut body = Body::dummy(dummy_def_id());
        let param_ty = TyKind::Param(ParamTy { index: 0, name: "T".into() });
        let param_ty = ctx_mut.mk_ty(param_ty);
        body.return_ty = param_ty;
        // Freeze the context
        let ty_ctx = ctx_mut.freeze();

        let substituted = substitute_body(&body, &substs, &ty_ctx);
        // Check that return_ty is now i32
        match ty_ctx.ty_kind(substituted.return_ty) {
            TyKind::Int(IntTy::I32) => (),
            _ => panic!("return_ty not replaced with i32"),
        }
    });
}

#[test]
fn substitute_body_with_empty_substitution_no_change() {
    let (ty_ctx, _) = with_fresh_ty_ctx(|ctx_mut| {
        let substs = Substitution::empty();
        let mut body = Body::dummy(dummy_def_id());
        let param_ty = TyKind::Param(ParamTy { index: 0, name: "T".into() });
        let param_ty = ctx_mut.mk_ty(param_ty);
        body.return_ty = param_ty;
        let ty_ctx = ctx_mut.freeze();
        let substituted = substitute_body(&body, &substs, &ty_ctx);
        // Should still be ParamTy (no substitution)
        match ty_ctx.ty_kind(substituted.return_ty) {
            TyKind::Param(_) => (),
            _ => panic!("return_ty changed when it shouldn't"),
        }
    });
}
