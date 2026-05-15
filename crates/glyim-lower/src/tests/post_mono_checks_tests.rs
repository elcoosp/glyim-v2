use glyim_core::arena::IndexVec;
use glyim_core::def_id::{CrateId, DefId, FnDefId, LocalDefId};
use glyim_core::primitives::Mutability;
use glyim_mir::{
    Body, LocalDecl, SourceInfo, Terminator, TerminatorKind,
};
use glyim_span::Span;
use glyim_diag::DiagSeverity;
use glyim_test::{assert_diag_contains, assert_error_count, assert_has_errors, assert_has_severity, assert_no_errors};
use glyim_type::{GenericArg, Region, Substitution, Ty, TyKind};
use std::sync::Arc;

use crate::mono::{MonoItem, MonoItemData};
use crate::post_mono_checks::{
    check_large_mono_set, check_unsized_locals, check_unused_generic_params,
};

/// Helper to create a minimal MIR body with given locals and a single return block.
fn make_body(owner_def_id: DefId, return_ty: Ty, locals_ty: Vec<Ty>) -> Body {
    let mut locals = IndexVec::new();
    // _0 is return place
    locals.push(LocalDecl {
        ty: return_ty,
        mutability: Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    for ty in locals_ty {
        locals.push(LocalDecl {
            ty,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });
    }

    let mut basic_blocks = IndexVec::new();
    let return_bb = basic_blocks.push(glyim_mir::BasicBlockData {
        statements: vec![],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    });
    let _entry_bb = basic_blocks.push(glyim_mir::BasicBlockData {
        statements: vec![],
        terminator: Terminator {
            kind: TerminatorKind::Goto { target: return_bb },
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    });

    Body {
        owner: owner_def_id,
        basic_blocks,
        locals,
        arg_count: 0,
        return_ty,
        span: Span::DUMMY,
        var_debug_info: vec![],
    }
}

fn make_fn_def_id(raw: u32) -> FnDefId {
    FnDefId::from_raw(raw)
}

fn make_def_id(raw: u32) -> DefId {
    DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(raw))
}

/// Create a MonoItemData for a function with the given substitution.
fn make_mono_fn_with_subst(def_id_raw: u32, subst: Substitution, body: Body) -> MonoItemData {
    MonoItemData {
        item: MonoItem::Fn {
            def_id: make_fn_def_id(def_id_raw),
            substs: subst,
        },
        body: Arc::new(body),
        symbol: format!("fn_{}", def_id_raw),
        source_module: 0,
    }
}

// ==================== V26-T02: Unsized locals ====================

#[test]
fn unsized_local_errors() {
    let mut ctx_mut = glyim_test::test_ty_ctx();
    let slice_ty = ctx_mut.mk_ty(TyKind::Slice(Ty::UNIT));
    let frozen = ctx_mut.freeze();

    let body = make_body(make_def_id(1), Ty::UNIT, vec![slice_ty]);
    let item = make_mono_fn_with_subst(1, Substitution::empty(), body);
    let items = vec![item];

    let diags = check_unsized_locals(&items, &frozen);
    assert_has_errors(&diags);
    assert_error_count(&diags, 1);
    assert_diag_contains(&diags, "unsized local");
}

// ==================== V26-T03: Large mono item set ====================

#[test]
fn large_mono_set_warns() {
    let body = make_body(make_def_id(0), Ty::UNIT, vec![]);
    let mut items = Vec::new();
    for i in 0..50 {
        items.push(make_mono_fn_with_subst(i, Substitution::empty(), body.clone()));
    }
    let threshold = 30;
    let diags = check_large_mono_set(&items, threshold);
    assert_has_severity(&diags, DiagSeverity::Warning);
    assert_diag_contains(&diags, "large number of mono items");
}

#[test]
fn small_mono_set_no_warn() {
    let body = make_body(make_def_id(0), Ty::UNIT, vec![]);
    let items: Vec<MonoItemData> = (0..5)
        .map(|i| make_mono_fn_with_subst(i, Substitution::empty(), body.clone()))
        .collect();
    let diags = check_large_mono_set(&items, 100);
    assert_no_errors(&diags);
}

// ==================== V26-T01: Unused generic params ====================

#[test]
fn unused_generic_param_warns() {
    let mut ctx_mut = glyim_test::test_ty_ctx();
    let subst = ctx_mut.intern_substitution(vec![GenericArg::Ty(Ty::UNIT)]);
    let frozen = ctx_mut.freeze();

    let body = make_body(make_def_id(10), Ty::UNIT, vec![]);
    let item = make_mono_fn_with_subst(10, subst, body);
    let items = vec![item];

    let diags = check_unused_generic_params(&items, &frozen);
    assert_has_severity(&diags, DiagSeverity::Warning);
    assert_diag_contains(&diags, "unused generic parameter");
}

#[test]
fn no_unused_when_body_uses_generic() {
    let mut ctx_mut = glyim_test::test_ty_ctx();
    let name = ctx_mut.resolver().intern("T");
    let used_ty = ctx_mut.mk_ty(TyKind::Param(glyim_type::ParamTy {
        index: 0,
        name,
    }));
    let subst = ctx_mut.intern_substitution(vec![GenericArg::Ty(used_ty)]);
    let frozen = ctx_mut.freeze();

    let body = make_body(make_def_id(11), Ty::UNIT, vec![used_ty]);
    let item = make_mono_fn_with_subst(11, subst, body);
    let items = vec![item];

    let diags = check_unused_generic_params(&items, &frozen);
    assert_no_errors(&diags);
}


// ==================== Extended V26-T02: Unsized locals ====================

#[test]
fn unsized_dynamic_errors() {
    let mut ctx_mut = glyim_test::test_ty_ctx();
    // Create a Dynamic (trait object) type
    let predicates: Vec<glyim_type::Predicate> = vec![
        glyim_type::Predicate::WellFormed(Ty::UNIT),
    ];
    let binder = glyim_type::Binder::bind(
        predicates.into_boxed_slice(),
        vec![].into_boxed_slice(),
    );
    let dyn_ty = ctx_mut.mk_ty(TyKind::Dynamic(
        binder,
        Region::Erased,
    ));
    let frozen = ctx_mut.freeze();

    let body = make_body(make_def_id(2), Ty::UNIT, vec![dyn_ty]);
    let item = make_mono_fn_with_subst(2, Substitution::empty(), body);
    let items = vec![item];

    let diags = check_unsized_locals(&items, &frozen);
    assert_has_errors(&diags);
    assert_error_count(&diags, 1);
    assert_diag_contains(&diags, "unsized local");
}

#[test]
fn multiple_unsized_locals_multiple_errors() {
    let mut ctx_mut = glyim_test::test_ty_ctx();
    let slice_ty1 = ctx_mut.mk_ty(TyKind::Slice(Ty::UNIT));
    let slice_ty2 = ctx_mut.mk_ty(TyKind::Slice(Ty::BOOL));
    let frozen = ctx_mut.freeze();

    let body = make_body(make_def_id(3), Ty::UNIT, vec![slice_ty1, slice_ty2]);
    let item = make_mono_fn_with_subst(3, Substitution::empty(), body);
    let items = vec![item];

    let diags = check_unsized_locals(&items, &frozen);
    assert_has_errors(&diags);
    assert_error_count(&diags, 2);
}

#[test]
fn sized_locals_no_error() {
    let mut ctx_mut = glyim_test::test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(glyim_core::primitives::IntTy::I32));
    let bool_ty = Ty::BOOL;
    let unit_ty = Ty::UNIT;
    let frozen = ctx_mut.freeze();

    let body = make_body(make_def_id(4), Ty::UNIT, vec![i32_ty, bool_ty, unit_ty]);
    let item = make_mono_fn_with_subst(4, Substitution::empty(), body);
    let items = vec![item];

    let diags = check_unsized_locals(&items, &frozen);
    assert_no_errors(&diags);
}

// ==================== Extended V26-T03: Large mono item set ====================

#[test]
fn large_mono_set_exactly_at_threshold_no_warn() {
    let body = make_body(make_def_id(0), Ty::UNIT, vec![]);
    let items: Vec<MonoItemData> = (0..30)
        .map(|i| make_mono_fn_with_subst(i, Substitution::empty(), body.clone()))
        .collect();
    let threshold = 30;
    let diags = check_large_mono_set(&items, threshold);
    assert_no_errors(&diags);
}

#[test]
fn large_mono_set_one_above_threshold_warns() {
    let body = make_body(make_def_id(0), Ty::UNIT, vec![]);
    let items: Vec<MonoItemData> = (0..31)
        .map(|i| make_mono_fn_with_subst(i, Substitution::empty(), body.clone()))
        .collect();
    let threshold = 30;
    let diags = check_large_mono_set(&items, threshold);
    assert_has_severity(&diags, DiagSeverity::Warning);
}

#[test]
fn empty_mono_set_no_warn() {
    let items: Vec<MonoItemData> = vec![];
    let diags = check_large_mono_set(&items, 1);
    assert_no_errors(&diags);
}

// ==================== Extended V26-T01: Unused generic params ====================

#[test]
fn unused_generic_param_multiple_substs_warns() {
    let mut ctx_mut = glyim_test::test_ty_ctx();
    let subst = ctx_mut.intern_substitution(vec![
        GenericArg::Ty(Ty::UNIT),
        GenericArg::Ty(Ty::BOOL),
    ]);
    let frozen = ctx_mut.freeze();

    let body = make_body(make_def_id(12), Ty::UNIT, vec![]);
    let item = make_mono_fn_with_subst(12, subst, body);
    let items = vec![item];

    let diags = check_unused_generic_params(&items, &frozen);
    assert_has_severity(&diags, DiagSeverity::Warning);
    assert_diag_contains(&diags, "unused generic parameter");
}

#[test]
fn empty_substitution_no_warn() {
    let ctx_mut = glyim_test::test_ty_ctx();
    let subst = Substitution::empty();
    let frozen = ctx_mut.freeze();

    let body = make_body(make_def_id(13), Ty::UNIT, vec![]);
    let item = make_mono_fn_with_subst(13, subst, body);
    let items = vec![item];

    let diags = check_unused_generic_params(&items, &frozen);
    assert_no_errors(&diags);
}

#[test]
fn generic_param_used_in_rvalue_constant_no_warn() {
    let mut ctx_mut = glyim_test::test_ty_ctx();
    let param_ty = ctx_mut.mk_ty(TyKind::Param(glyim_type::ParamTy {
        index: 0,
        name: ctx_mut.resolver().intern("T"),
    }));
    let subst = ctx_mut.intern_substitution(vec![GenericArg::Ty(param_ty)]);
    let frozen = ctx_mut.freeze();

    // Body uses the param type in a local
    let body = make_body(make_def_id(14), Ty::UNIT, vec![param_ty]);
    let item = make_mono_fn_with_subst(14, subst, body);
    let items = vec![item];

    let diags = check_unused_generic_params(&items, &frozen);
    assert_no_errors(&diags);
}

