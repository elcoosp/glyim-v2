use glyim_core::arena::IndexVec;
use glyim_core::def_id::{CrateId, DefId, FnDefId, LocalDefId};
use glyim_core::primitives::Mutability;
use glyim_mir::{
    Body, LocalDecl, SourceInfo, Terminator, TerminatorKind,
};
use glyim_span::Span;
use glyim_test::{assert_diag_contains, assert_error_count, assert_has_errors, assert_no_errors};
use glyim_type::{GenericArg, Substitution, Ty, TyKind};
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
    assert_has_errors(&diags);
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
    // Create a substitution with one generic arg (e.g., i32) that is not used in the body
    let mut ctx_mut = glyim_test::test_ty_ctx();
    let subst = ctx_mut.intern_substitution(vec![GenericArg::Ty(Ty::UNIT)]);
    let _frozen = ctx_mut.freeze(); // not needed for this check, but we might need a ctx

    let body = make_body(make_def_id(10), Ty::UNIT, vec![]); // no locals use any generic
    let item = make_mono_fn_with_subst(10, subst, body);
    let items = vec![item];

    let diags = check_unused_generic_params(&items);
    // Should warn about unused generic parameter(s)
    assert_has_errors(&diags);
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
    let _frozen = ctx_mut.freeze();

    // Body local type uses the generic param (index 0)
    let body = make_body(make_def_id(11), Ty::UNIT, vec![used_ty]);
    let item = make_mono_fn_with_subst(11, subst, body);
    let items = vec![item];

    let diags = check_unused_generic_params(&items);
    assert_no_errors(&diags);
}
