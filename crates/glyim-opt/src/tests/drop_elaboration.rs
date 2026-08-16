//! Tests for drop elaboration: conditional drops, array loops, enum discriminants.

use glyim_core::primitives::Mutability;
use glyim_core::{CrateId, DefId, IndexVec, LocalDefId};
use glyim_mir::*;
use glyim_span::Span;
use glyim_type::{Const, ConstKind, Ty, TyCtxMut, TyKind};

// Helper to create a body with an array of a type that needs drop.
// Now takes a mutable context and uses it to build the types.
fn body_with_array_drop(ctx: &mut TyCtxMut, elem_ty: Ty, len: u64) -> Body {
    let const_len = Const {
        kind: ConstKind::Uint(len.into()),
        ty: ctx.mk_ty(TyKind::Uint(glyim_core::UintTy::Usize)),
    };
    let array_ty = ctx.mk_ty(TyKind::Array(elem_ty, const_len));
    let mut body = Body::dummy(DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));
    let local = body.locals.push(LocalDecl {
        ty: array_ty,
        mutability: Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    let stmt1 = Statement {
        kind: StatementKind::StorageLive(local),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let term = Terminator {
        kind: TerminatorKind::Drop {
            place: Place::new(local),
            target: BasicBlockIdx::from_raw(1),
            cleanup: None,
        },
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let block0 = BasicBlockData {
        statements: vec![stmt1],
        terminator: term,
        is_cleanup: false,
    };
    let return_term = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let block1 = BasicBlockData {
        statements: vec![],
        terminator: return_term,
        is_cleanup: false,
    };
    body.basic_blocks = IndexVec::from_raw(vec![block0, block1]);
    body
}
#[test]
fn array_drop_creates_loop() {
    use glyim_core::AdtId;
    let mut ctx_mut = glyim_test::test_ty_ctx();
    // Create a type that needs drop (a struct with a String field)
    let string_ty = ctx_mut.mk_ty(TyKind::String);
    let adt_id = AdtId::from_raw(100);
    let subst = ctx_mut.intern_substitution(vec![]);
    // Register a struct with one field: String
    let field_defs = glyim_core::arena::IndexVec::from_raw(vec![glyim_type::FieldDef {
        name: ctx_mut.resolver().intern("s"),
        ty: string_ty,
    }]);
    let variant = glyim_type::VariantDef {
        name: ctx_mut.resolver().intern("S"),
        fields: field_defs.clone(),
    };
    let adt_def = glyim_type::AdtDef {
        kind: glyim_type::AdtKind::Struct,
        fields: field_defs.clone(),
        variants: vec![variant],
    };
    ctx_mut.register_adt(adt_id, adt_def);
    let struct_ty = ctx_mut.mk_ty(TyKind::Adt(adt_id, subst));

    // Build array of that struct using the same context
    let len = 3;
    let mut body = body_with_array_drop(&mut ctx_mut, struct_ty, len);

    // Freeze the context after the body is built
    let ctx = ctx_mut.freeze();

    // Now run drop elaboration
    crate::elaborate_drops(&ctx, &mut body);

    // Assert that the Drop terminator is replaced by a loop (SwitchInt + Goto)
    let has_loop = body
        .basic_blocks
        .iter()
        .any(|block| matches!(block.terminator.kind, TerminatorKind::SwitchInt { .. }));
    assert!(has_loop, "Expected a SwitchInt loop for array drop");
    // Also ensure there is no direct Drop terminator on the array itself
    let has_array_drop = body.basic_blocks.iter().any(|block| {
        if let TerminatorKind::Drop { place, .. } = &block.terminator.kind {
            place.projection.is_empty()
        } else {
            false
        }
    });
    assert!(
        !has_array_drop,
        "Array Drop terminator should have been replaced"
    );
}

#[test]
fn drop_on_projected_place_is_not_skipped() {
    // A Drop on a place with a projection (e.g. `*p`) must still drop the
    // value at that place. Regression guard for the old `Goto` stub in
    // drop_elaboration that silently skipped drops on projected places.
    let mut ctx_mut = glyim_test::test_ty_ctx();
    // Use a String (needs drop) behind a local, and drop through a Deref.
    let string_ty = ctx_mut.mk_ty(TyKind::String);
    let ref_string_ty = ctx_mut.mk_ty(TyKind::Ref(
        glyim_type::Region::Erased,
        string_ty,
        Mutability::Mut,
    ));
    let ctx = ctx_mut.freeze();

    let body_def = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0));
    let mut body = Body::dummy(body_def);
    let local = body.locals.push(LocalDecl {
        ty: ref_string_ty,
        mutability: Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    let drop_place = Place {
        local,
        projection: Box::new([ProjectionElem::Deref]),
    };
    let block0 = BasicBlockData {
        statements: vec![],
        terminator: Terminator {
            kind: TerminatorKind::Drop {
                place: drop_place,
                target: BasicBlockIdx::from_raw(1),
                cleanup: None,
            },
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    };
    let block1 = BasicBlockData {
        statements: vec![],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    };
    body.basic_blocks = IndexVec::from_raw(vec![block0, block1]);

    crate::drop_elaboration::run(&ctx, &mut body);

    // The elaborated block 0 must still be a Drop (not a Goto that skips it).
    let is_drop = matches!(body.basic_blocks[BasicBlockIdx::from_raw(0)].terminator.kind, TerminatorKind::Drop { .. });
    assert!(
        is_drop,
        "Drop on a projected place must NOT be turned into Goto (would leak)"
    );
}
