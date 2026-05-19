use crate::*;
use glyim_core::arena::IndexVec;
use glyim_type::*;
use glyim_test::{assert_ty, with_fresh_ty_ctx};

#[test]
fn place_ty_deref_then_field_on_tuple() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        // Build substitution components first
        let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
        let bool_ty = ctx_mut.bool_ty();
        let subst = ctx_mut.intern_substitution(vec![
            GenericArg::Ty(i32_ty),
            GenericArg::Ty(bool_ty),
        ]);
        let inner_ty = ctx_mut.mk_ty(TyKind::Tuple(subst));
        let ref_ty = ctx_mut.mk_ref(Region::Erased, inner_ty, Mutability::Not);
        let mut locals = IndexVec::new();
        locals.push(LocalDecl {
            ty: ref_ty,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });
        Body {
            owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
            basic_blocks: IndexVec::new(),
            locals,
            arg_count: 0,
            return_ty: Ty::ERROR,
            span: Span::DUMMY,
            var_debug_info: Vec::new(),
        }
    });
    let place = Place {
        local: LocalIdx::from_raw(0),
        projection: Box::new([ProjectionElem::Deref, ProjectionElem::Field(FieldIdx::from_raw(0))]),
    };
    let ty = place.ty(&ctx, &body.locals);
    assert_ty(&ctx, ty).is_int(IntTy::I32);
}

#[test]
fn place_ty_field_on_tuple() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let i64_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I64));
        let bool_ty = ctx_mut.bool_ty();
        let subst = ctx_mut.intern_substitution(vec![
            GenericArg::Ty(i64_ty),
            GenericArg::Ty(bool_ty),
        ]);
        let tuple_ty = ctx_mut.mk_ty(TyKind::Tuple(subst));
        let mut locals = IndexVec::new();
        locals.push(LocalDecl {
            ty: tuple_ty,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });
        Body {
            owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
            basic_blocks: IndexVec::new(),
            locals,
            arg_count: 0,
            return_ty: Ty::ERROR,
            span: Span::DUMMY,
            var_debug_info: Vec::new(),
        }
    });
    let place = Place {
        local: LocalIdx::from_raw(0),
        projection: Box::new([ProjectionElem::Field(FieldIdx::from_raw(1))]),
    };
    let ty = place.ty(&ctx, &body.locals);
    assert_ty(&ctx, ty).is_bool();
}
