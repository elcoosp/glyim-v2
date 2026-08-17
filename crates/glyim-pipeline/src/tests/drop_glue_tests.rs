use glyim_core::primitives::{IntTy, UintTy};
use glyim_mir::TerminatorKind;
use glyim_test::with_fresh_ty_ctx;
use glyim_type::{Const, ConstKind, TyKind};

#[test]
fn drop_glue_for_i32_generates_body() {
    let (ty_ctx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Int(IntTy::I32)));
    let body = crate::mono_cache::generate_drop_glue(ty, &ty_ctx);
    // Just verify the body has at least one block (no panic)
    assert!(!body.basic_blocks.is_empty());
}

#[test]
fn drop_glue_for_bool_generates_body() {
    let (ty_ctx, ty) = with_fresh_ty_ctx(|c| c.bool_ty());
    let body = crate::mono_cache::generate_drop_glue(ty, &ty_ctx);
    assert!(!body.basic_blocks.is_empty());
}

#[test]
fn drop_glue_for_array_of_droppable_drops_each_element() {
    // `[String; 5]`: each element needs drop, so the generated glue must contain
    // exactly 5 `Drop` terminators (one per element index), not a bare `Return`.
    // This is the de-stubbing-plan §16.1 fix — previously arrays produced a
    // single `Return` and silently skipped every element's destructor.
    let (ty_ctx, ty) = with_fresh_ty_ctx(|c| {
        let string_ty = c.mk_ty(TyKind::String);
        let usize_ty = c.mk_ty(TyKind::Uint(UintTy::Usize));
        let len = Const {
            kind: ConstKind::Uint(5),
            ty: usize_ty,
        };
        c.mk_ty(TyKind::Array(string_ty, len))
    });
    let body = crate::mono_cache::generate_drop_glue(ty, &ty_ctx);

    let mut drop_terminators = 0usize;
    let mut indexed_drops = 0usize;
    for bb in body.basic_blocks.iter() {
        if let TerminatorKind::Drop { place, .. } = &bb.terminator.kind {
            drop_terminators += 1;
            if place
                .projection
                .iter()
                .any(|p| matches!(p, glyim_mir::ProjectionElem::Index(_)))
            {
                indexed_drops += 1;
            }
        }
    }
    assert_eq!(
        drop_terminators, 5,
        "expected 5 Drop terminators (one per element)"
    );
    assert_eq!(
        indexed_drops, 5,
        "each Drop must target a distinct array element"
    );
}

