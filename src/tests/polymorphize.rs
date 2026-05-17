//! Tests for polymorphization splitting.

use glyim_core::arena::IndexVec;
use glyim_core::def_id::{DefId, FnDefId, LocalDefId, CrateId};
use glyim_mir::{self, BasicBlockIdx, LocalIdx, Statement, Terminator, SourceInfo, StatementKind, TerminatorKind, Operand, Rvalue, MirConst, MirConstKind, Place, ProjectionElem};
use glyim_span::{Span, FileId, ByteIdx, SyntaxContext};
use glyim_type::{Ty, TyKind, Substitution, GenericArg, Const, ConstKind, ParamTy, TypeLookup};
use glyim_test::{test_ty_ctx, with_fresh_ty_ctx};

use crate::polymorphize::{analyze_used_params, polymorphize_substs, compute_poly_item, deduplicate};
use crate::mono::{MonoItem, MonoItemData};

fn dummy_body() -> mir::Body {
    let owner = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0));
    mir::Body {
        owner,
        basic_blocks: IndexVec::new(),
        locals: IndexVec::new(),
        arg_count: 0,
        return_ty: Ty::UNIT,
        span: Span::DUMMY,
        var_debug_info: vec![],
    }
}

#[test]
fn test_analyze_used_params_none_used() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let mut b = dummy_body();

        // Create a generic param T at index 0
        let param_ty = ctx_mut.mk_ty(TyKind::Param(ParamTy { index: 0, name: glyim_core::interner::Name::new("T") }));

        // Add a local, but make it i32 (not using the param)
        let i32_ty = ctx_mut.mk_ty(TyKind::Int(glyim_core::primitives::IntTy::I32));
        b.locals.push(mir::LocalDecl {
            ty: i32_ty,
            mutability: glyim_core::primitives::Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });

        let substs = ctx_mut.intern_substitution(vec![GenericArg::Ty(param_ty)]);
        (b, substs)
    });

    // The param is not used in the body (local is i32, not T)
    let used = analyze_used_params(&body, &ctx, body);
    assert_eq!(used, vec![false]);
}

#[test]
fn test_analyze_used_params_one_used() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let mut b = dummy_body();

        // Create a generic param T at index 0
        let param_ty = ctx_mut.mk_ty(TyKind::Param(ParamTy { index: 0, name: glyim_core::interner::Name::new("T") }));

        // Add a local OF type T
        b.locals.push(mir::LocalDecl {
            ty: param_ty,
            mutability: glyim_core::primitives::Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });

        let substs = ctx_mut.intern_substitution(vec![GenericArg::Ty(param_ty)]);
        (b, substs)
    });

    let used = analyze_used_params(&body, &ctx, body);
    assert_eq!(used, vec![true]);
}

#[test]
fn test_polymorphize_substs_replaces_unused() {
    let (ctx, substs) = with_fresh_ty_ctx(|ctx_mut| {
        let param_ty = ctx_mut.mk_ty(TyKind::Param(ParamTy { index: 0, name: glyim_core::interner::Name::new("T") }));
        ctx_mut.intern_substitution(vec![GenericArg::Ty(param_ty)])
    });

    let used = vec![false];
    let poly_substs = polymorphize_substs(&ctx, substs, &used);

    let args = ctx.substitution_args(poly_substs);
    assert_eq!(args.len(), 1);
    match &args[0] {
        GenericArg::Ty(ty) => {
            assert_eq!(ctx.ty_kind(*ty), &TyKind::Unit);
        }
        _ => panic!("Expected Ty"),
    }
}

#[test]
fn test_polymorphize_substs_keeps_used() {
    let (ctx, param_ty) = with_fresh_ty_ctx(|ctx_mut| {
        let ty = ctx_mut.mk_ty(TyKind::Param(ParamTy { index: 0, name: glyim_core::interner::Name::new("T") }));
        (ctx_mut.freeze(), ty)
    });

    let mut ctx_mut = test_ty_ctx();
    let substs = ctx_mut.intern_substitution(vec![GenericArg::Ty(param_ty)]);

    let used = vec![true];
    let poly_substs = polymorphize_substs(&mut ctx_mut, substs, &used);

    // Re-freeze to check
    let ctx = ctx_mut.freeze();
    let args = ctx.substitution_args(poly_substs);
    assert_eq!(args.len(), 1);
    match &args[0] {
        GenericArg::Ty(ty) => {
            assert!(matches!(ctx.ty_kind(*ty), TyKind::Param(_)));
        }
        _ => panic!("Expected Ty"),
    }
}

#[test]
fn test_compute_poly_item_fn() {
    let (ctx, item) = with_fresh_ty_ctx(|ctx_mut| {
        let mut b = dummy_body();
        let def_id = FnDefId::from_raw(0);

        // Param T is unused in body
        let i32_ty = ctx_mut.mk_ty(TyKind::Int(glyim_core::primitives::IntTy::I32));
        b.locals.push(mir::LocalDecl {
            ty: i32_ty,
            mutability: glyim_core::primitives::Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });

        let param_ty = ctx_mut.mk_ty(TyKind::Param(ParamTy { index: 0, name: glyim_core::interner::Name::new("T") }));
        let substs = ctx_mut.intern_substitution(vec![GenericArg::Ty(param_ty)]);

        let item = MonoItem::Fn { def_id, substs };
        (b, item)
    });

    let mut ctx_mut = test_ty_ctx();
    let poly_item = compute_poly_item(&mut ctx_mut, &item, &ctx);

    let frozen = ctx_mut.freeze();

    match poly_item {
        MonoItem::Fn { def_id, substs } => {
            assert_eq!(def_id.to_raw(), 0);
            let args = frozen.substitution_args(substs);
            assert_eq!(args.len(), 1);
            // Should be Unit because param was unused
            match &args[0] {
                GenericArg::Ty(ty) => assert_eq!(frozen.ty_kind(*ty), &TyKind::Unit),
                _ => panic!("Expected Ty"),
            }
        }
        _ => panic!("Expected Fn"),
    }
}

#[test]
fn test_deduplicate_removes_unused_param_variants() {
    let (ctx, items) = with_fresh_ty_ctx(|ctx_mut| {
        let def_id = FnDefId::from_raw(0);
        let mut b = dummy_body();

        // Param T unused
        let i32_ty = ctx_mut.mk_ty(TyKind::Int(glyim_core::primitives::IntTy::I32));
        b.locals.push(mir::LocalDecl {
            ty: i32_ty,
            mutability: glyim_core::primitives::Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });

        // Create two items with different concrete types for T, but T is unused
        // They should dedup to the same item (Unit)
        let t1 = ctx_mut.mk_ty(TyKind::Int(glyim_core::primitives::IntTy::I32));
        let t2 = ctx_mut.mk_ty(TyKind::Bool);

        let substs1 = ctx_mut.intern_substitution(vec![GenericArg::Ty(t1)]);
        let substs2 = ctx_mut.intern_substitution(vec![GenericArg::Ty(t2)]);

        let item1 = MonoItemData {
            item: MonoItem::Fn { def_id, substs: substs1 },
            body: std::sync::Arc::new(b.clone()),
            symbol: "foo_i32".to_string(),
            source_module: 0,
        };

        let item2 = MonoItemData {
            item: MonoItem::Fn { def_id, substs: substs2 },
            body: std::sync::Arc::new(b),
            symbol: "foo_bool".to_string(),
            source_module: 0,
        };

        (ctx_mut.freeze(), vec![item1, item2])
    });

    let mut ctx_mut = test_ty_ctx();
    let deduped = deduplicate(&mut ctx_mut, &items);

    // Should be reduced to 1 item
    assert_eq!(deduped.len(), 1);
    // Both original symbols were different, but we take the first one's metadata usually
    // or merge. The implementation keeps the first.
    assert_eq!(deduped[0].symbol, "foo_i32");

    let frozen = ctx_mut.freeze();
    match &deduped[0].item {
        MonoItem::Fn { substs, .. } => {
            let args = frozen.substitution_args(*substs);
            assert_eq!(args.len(), 1);
            match &args[0] {
                GenericArg::Ty(ty) => assert_eq!(frozen.ty_kind(*ty), &TyKind::Unit),
                _ => panic!(),
            }
        }
        _ => panic!(),
    }
}
