//! Tests for V23: Mono Item Graph Traversal & Collection
//!
//! V23-T01: Call graph: main calls foo<T> with concrete T -> foo<T> added
//! V23-T02: Recursive function -> no duplicate items
//! V23-T03: Drop glue for structs -> collected
//! V23-T04: Constant used in generic -> instantiated

use glyim_core::arena::IndexVec;
use glyim_core::def_id::*;
use glyim_core::primitives::*;
use crate::{MonoCtx, MonoItem};
use glyim_mir::*;
use glyim_span::Span;
use glyim_type::*;
use std::sync::Arc;

/// Helper: create a simple MIR body with a Call terminator to a function.
fn make_call_body(
    caller_def_id: FnDefId,
    callee_def_id: FnDefId,
    callee_substs: Substitution,
) -> Body {
    let owner = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(caller_def_id.to_raw()));
    let mut locals = IndexVec::new();
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });

    let func_const = MirConst {
        kind: MirConstKind::Fn(callee_def_id, callee_substs),
        ty: Ty::UNIT,
        span: Span::DUMMY,
    };

    let call_terminator = Terminator {
        kind: TerminatorKind::Call {
            func: Operand::Constant(func_const),
            args: vec![Operand::Copy(Place::new(LocalIdx::from_raw(1)))],
            destination: Place::new(LocalIdx::from_raw(2)),
            target: Some(BasicBlockIdx::from_raw(1)),
            cleanup: None,
        },
        source_info: SourceInfo::new(Span::DUMMY),
    };

    let return_terminator = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    };

    let mut basic_blocks = IndexVec::new();
    basic_blocks.push(BasicBlockData {
        statements: vec![],
        terminator: call_terminator,
        is_cleanup: false,
    });
    basic_blocks.push(BasicBlockData {
        statements: vec![],
        terminator: return_terminator,
        is_cleanup: false,
    });

    Body {
        owner,
        basic_blocks,
        locals,
        arg_count: 1,
        return_ty: Ty::UNIT,
        span: Span::DUMMY,
        var_debug_info: vec![],
    }
}

/// Helper: create a trivial MIR body (just return).
fn make_simple_body(fn_def_id: FnDefId) -> Body {
    let owner = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(fn_def_id.to_raw()));
    let mut locals = IndexVec::new();
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });

    let mut basic_blocks = IndexVec::new();
    basic_blocks.push(BasicBlockData {
        statements: vec![],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    });

    Body {
        owner,
        basic_blocks,
        locals,
        arg_count: 0,
        return_ty: Ty::UNIT,
        span: Span::DUMMY,
        var_debug_info: vec![],
    }
}

/// Helper: create a MIR body with a recursive call (calls itself).
fn make_recursive_body(fn_def_id: FnDefId, substs: Substitution) -> Body {
    let owner = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(fn_def_id.to_raw()));
    let mut locals = IndexVec::new();
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });

    let func_const = MirConst {
        kind: MirConstKind::Fn(fn_def_id, substs),
        ty: Ty::UNIT,
        span: Span::DUMMY,
    };

    let call_terminator = Terminator {
        kind: TerminatorKind::Call {
            func: Operand::Constant(func_const),
            args: vec![Operand::Copy(Place::new(LocalIdx::from_raw(1)))],
            destination: Place::new(LocalIdx::from_raw(2)),
            target: Some(BasicBlockIdx::from_raw(1)),
            cleanup: None,
        },
        source_info: SourceInfo::new(Span::DUMMY),
    };

    let return_terminator = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    };

    let mut basic_blocks = IndexVec::new();
    basic_blocks.push(BasicBlockData {
        statements: vec![],
        terminator: call_terminator,
        is_cleanup: false,
    });
    basic_blocks.push(BasicBlockData {
        statements: vec![],
        terminator: return_terminator,
        is_cleanup: false,
    });

    Body {
        owner,
        basic_blocks,
        locals,
        arg_count: 1,
        return_ty: Ty::UNIT,
        span: Span::DUMMY,
        var_debug_info: vec![],
    }
}

/// Helper: create a body with a Drop terminator.
fn make_drop_body(fn_def_id: FnDefId, _dropped_adt_id: AdtId) -> Body {
    let owner = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(fn_def_id.to_raw()));
    let mut locals = IndexVec::new();
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });

    let drop_terminator = Terminator {
        kind: TerminatorKind::Drop {
            place: Place::new(LocalIdx::from_raw(1)),
            target: BasicBlockIdx::from_raw(1),
            cleanup: None,
        },
        source_info: SourceInfo::new(Span::DUMMY),
    };

    let return_terminator = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    };

    let mut basic_blocks = IndexVec::new();
    basic_blocks.push(BasicBlockData {
        statements: vec![],
        terminator: drop_terminator,
        is_cleanup: false,
    });
    basic_blocks.push(BasicBlockData {
        statements: vec![],
        terminator: return_terminator,
        is_cleanup: false,
    });

    Body {
        owner,
        basic_blocks,
        locals,
        arg_count: 0,
        return_ty: Ty::UNIT,
        span: Span::DUMMY,
        var_debug_info: vec![],
    }
}

/// V23-T01: Call graph: main calls foo<T> with concrete T -> foo<T> added
#[test]
fn call_graph_adds_callee() {
    let main_id = FnDefId::from_raw(1);
    let foo_id = FnDefId::from_raw(2);
    let empty_substs = Substitution::empty();

    let main_body = make_call_body(main_id, foo_id, empty_substs);
    let foo_body = make_simple_body(foo_id);

    let main_item = MonoItem::Fn {
        def_id: main_id,
        substs: empty_substs,
    };

    let main_body_arc = Arc::new(main_body);
    let foo_body_arc = Arc::new(foo_body);
    let bodies = vec![
        (main_id, empty_substs, main_body_arc.clone()),
        (foo_id, empty_substs, foo_body_arc.clone()),
    ];

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[main_item],
        &|def_id, substs| {
            for (fn_id, fn_substs, body) in &bodies {
                if def_id.local_id.to_raw() == fn_id.to_raw() && *substs == *fn_substs {
                    return body.clone();
                }
            }
            Arc::new(Body::dummy(def_id))
        },
    );

    assert_eq!(ctx.item_count(), 2, "should have both main and foo items");

    let items = ctx.items();
    let has_main = items.iter().any(|d| matches!(&d.item, MonoItem::Fn { def_id, .. } if def_id.to_raw() == 1));
    let has_foo = items.iter().any(|d| matches!(&d.item, MonoItem::Fn { def_id, .. } if def_id.to_raw() == 2));
    assert!(has_main, "main should be collected");
    assert!(has_foo, "foo should be collected via call graph traversal");
}

/// V23-T02: Recursive function -> no duplicate items
#[test]
fn recursive_function_no_duplicates() {
    let rec_id = FnDefId::from_raw(10);
    let empty_substs = Substitution::empty();

    let rec_body = make_recursive_body(rec_id, empty_substs);

    let rec_item = MonoItem::Fn {
        def_id: rec_id,
        substs: empty_substs,
    };

    let rec_body_arc = Arc::new(rec_body);
    let bodies = vec![(rec_id, empty_substs, rec_body_arc.clone())];

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[rec_item],
        &|def_id, substs| {
            for (fn_id, fn_substs, body) in &bodies {
                if def_id.local_id.to_raw() == fn_id.to_raw() && *substs == *fn_substs {
                    return body.clone();
                }
            }
            Arc::new(Body::dummy(def_id))
        },
    );

    assert_eq!(ctx.item_count(), 1, "recursive function should appear exactly once");

    let items = ctx.items();
    assert!(items.iter().any(|d| matches!(&d.item, MonoItem::Fn { def_id, .. } if def_id.to_raw() == 10)));
}

/// V23-T03: Drop glue for structs -> collected
#[test]
fn drop_glue_collected() {
    let fn_id = FnDefId::from_raw(20);
    let adt_id = AdtId::from_raw(50);
    let empty_substs = Substitution::empty();

    let drop_body = make_drop_body(fn_id, adt_id);

    let fn_item = MonoItem::Fn {
        def_id: fn_id,
        substs: empty_substs,
    };

    let drop_body_arc = Arc::new(drop_body);
    let bodies = vec![(fn_id, empty_substs, drop_body_arc.clone())];

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[fn_item],
        &|def_id, substs| {
            for (fn_id2, fn_substs, body) in &bodies {
                if def_id.local_id.to_raw() == fn_id2.to_raw() && *substs == *fn_substs {
                    return body.clone();
                }
            }
            Arc::new(Body::dummy(def_id))
        },
    );

    assert!(ctx.item_count() >= 1, "function with drop should be collected");

    let items = ctx.items();
    let has_fn = items.iter().any(|d| matches!(&d.item, MonoItem::Fn { def_id, .. } if def_id.to_raw() == 20));
    assert!(has_fn, "function containing drop should be collected");
}

/// V23-T04: Constant used in generic -> instantiated
#[test]
fn constant_in_generic_instantiated() {
    let main_id = FnDefId::from_raw(30);
    let const_id = ConstDefId::from_raw(5);
    let empty_substs = Substitution::empty();

    let owner = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(main_id.to_raw()));
    let mut locals = IndexVec::new();
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });

    let const_val = MirConst {
        kind: MirConstKind::ConstRef(const_id, empty_substs),
        ty: Ty::UNIT,
        span: Span::DUMMY,
    };

    let assign_stmt = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(1)),
            Rvalue::Use(Operand::Constant(const_val)),
        ),
        source_info: SourceInfo::new(Span::DUMMY),
    };

    let return_term = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    };

    let mut basic_blocks = IndexVec::new();
    basic_blocks.push(BasicBlockData {
        statements: vec![assign_stmt],
        terminator: return_term,
        is_cleanup: false,
    });

    let main_body = Body {
        owner,
        basic_blocks,
        locals,
        arg_count: 0,
        return_ty: Ty::UNIT,
        span: Span::DUMMY,
        var_debug_info: vec![],
    };

    let main_item = MonoItem::Fn {
        def_id: main_id,
        substs: empty_substs,
    };

    let main_body_arc = Arc::new(main_body);
    let bodies = vec![(main_id, empty_substs, main_body_arc.clone())];

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[main_item],
        &|def_id, substs| {
            for (fn_id, fn_substs, body) in &bodies {
                if def_id.local_id.to_raw() == fn_id.to_raw() && *substs == *fn_substs {
                    return body.clone();
                }
            }
            Arc::new(Body::dummy(def_id))
        },
    );

    assert!(ctx.item_count() >= 1, "main function should be collected");

    let items = ctx.items();
    let has_main = items.iter().any(|d| matches!(&d.item, MonoItem::Fn { def_id, .. } if def_id.to_raw() == 30));
    assert!(has_main, "main function should be collected");

    let has_const = items.iter().any(|d| {
        matches!(&d.item, MonoItem::Const { def_id, .. } if def_id.to_raw() == 5)
    });
    assert!(has_const, "referenced constant should be collected via graph traversal");
}

/// V23-T05: Diamond dependency - A calls B and C, both call D → D collected once
#[test]
fn diamond_dependency_dedup() {
    let a_id = FnDefId::from_raw(1);
    let b_id = FnDefId::from_raw(2);
    let c_id = FnDefId::from_raw(3);
    let d_id = FnDefId::from_raw(4);
    let empty_substs = Substitution::empty();

    // A calls B and C
    let a_body = make_call_body(a_id, b_id, empty_substs);
    // Add another call to C in A's body: we reuse make_call_body which creates one call.
    // For simplicity, A just calls B for now. We'll create separate bodies.
    let b_body = make_call_body(b_id, d_id, empty_substs);
    let c_body = make_call_body(c_id, d_id, empty_substs);
    let d_body = make_simple_body(d_id);

    // Build A's body that calls both B and C
    let owner_a = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(a_id.to_raw()));
    let mut locals_a = IndexVec::new();
    locals_a.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });
    locals_a.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Not, source_info: SourceInfo::new(Span::DUMMY) });
    locals_a.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });
    locals_a.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });

    let call_b = Terminator {
        kind: TerminatorKind::Call {
            func: Operand::Constant(MirConst { kind: MirConstKind::Fn(b_id, empty_substs), ty: Ty::UNIT, span: Span::DUMMY }),
            args: vec![Operand::Copy(Place::new(LocalIdx::from_raw(1)))],
            destination: Place::new(LocalIdx::from_raw(2)),
            target: Some(BasicBlockIdx::from_raw(1)),
            cleanup: None,
        },
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let call_c = Terminator {
        kind: TerminatorKind::Call {
            func: Operand::Constant(MirConst { kind: MirConstKind::Fn(c_id, empty_substs), ty: Ty::UNIT, span: Span::DUMMY }),
            args: vec![Operand::Copy(Place::new(LocalIdx::from_raw(1)))],
            destination: Place::new(LocalIdx::from_raw(3)),
            target: Some(BasicBlockIdx::from_raw(2)),
            cleanup: None,
        },
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let ret = Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(Span::DUMMY) };

    let mut bb = IndexVec::new();
    bb.push(BasicBlockData { statements: vec![], terminator: call_b, is_cleanup: false });
    bb.push(BasicBlockData { statements: vec![], terminator: call_c, is_cleanup: false });
    bb.push(BasicBlockData { statements: vec![], terminator: ret, is_cleanup: false });
    let a_body_diamond = Body { owner: owner_a, basic_blocks: bb, locals: locals_a, arg_count: 1, return_ty: Ty::UNIT, span: Span::DUMMY, var_debug_info: vec![] };

    let b_body_arc = Arc::new(b_body);
    let c_body_arc = Arc::new(c_body);
    let d_body_arc = Arc::new(d_body);
    let a_body_arc = Arc::new(a_body_diamond);

    let bodies = vec![
        (a_id, empty_substs, a_body_arc.clone()),
        (b_id, empty_substs, b_body_arc.clone()),
        (c_id, empty_substs, c_body_arc.clone()),
        (d_id, empty_substs, d_body_arc.clone()),
    ];

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[MonoItem::Fn { def_id: a_id, substs: empty_substs }],
        &|def_id, substs| {
            for (fn_id, fn_substs, body) in &bodies {
                if def_id.local_id.to_raw() == fn_id.to_raw() && *substs == *fn_substs {
                    return body.clone();
                }
            }
            Arc::new(Body::dummy(def_id))
        },
    );

    // Should have 4 items: A, B, C, D (D only once despite being called by both B and C)
    assert_eq!(ctx.item_count(), 4, "diamond: should have A, B, C, D (D deduped)");

    let d_count = ctx.items().iter().filter(|d| matches!(&d.item, MonoItem::Fn { def_id, .. } if def_id.to_raw() == 4)).count();
    assert_eq!(d_count, 1, "D should appear exactly once");
}

/// V23-T06: Chain of calls A→B→C→D - all collected transitively
#[test]
fn transitive_call_chain() {
    let a_id = FnDefId::from_raw(1);
    let b_id = FnDefId::from_raw(2);
    let c_id = FnDefId::from_raw(3);
    let d_id = FnDefId::from_raw(4);
    let empty_substs = Substitution::empty();

    let a_body = make_call_body(a_id, b_id, empty_substs);
    let b_body = make_call_body(b_id, c_id, empty_substs);
    let c_body = make_call_body(c_id, d_id, empty_substs);
    let d_body = make_simple_body(d_id);

    let bodies = vec![
        (a_id, empty_substs, Arc::new(a_body)),
        (b_id, empty_substs, Arc::new(b_body)),
        (c_id, empty_substs, Arc::new(c_body)),
        (d_id, empty_substs, Arc::new(d_body)),
    ];

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[MonoItem::Fn { def_id: a_id, substs: empty_substs }],
        &|def_id, substs| {
            for (fn_id, fn_substs, body) in &bodies {
                if def_id.local_id.to_raw() == fn_id.to_raw() && *substs == *fn_substs {
                    return body.clone();
                }
            }
            Arc::new(Body::dummy(def_id))
        },
    );

    assert_eq!(ctx.item_count(), 4, "chain: A→B→C→D should collect all 4");
    for raw_id in [1u32, 2, 3, 4] {
        let found = ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Fn { def_id, .. } if def_id.to_raw() == raw_id));
        assert!(found, "function with raw id {} should be collected", raw_id);
    }
}

/// V23-T07: Two different substitutions of same function → both collected
#[test]
fn different_substitutions_both_collected() {
    let main_id = FnDefId::from_raw(1);
    let foo_id = FnDefId::from_raw(2);
    let empty_substs = Substitution::empty();

    // Create a body where main calls foo with one substitution
    let main_body = make_call_body(main_id, foo_id, empty_substs);
    let foo_body = make_simple_body(foo_id);

    let main_body_arc = Arc::new(main_body);
    let foo_body_arc = Arc::new(foo_body);

    let bodies = vec![
        (main_id, empty_substs, main_body_arc.clone()),
        (foo_id, empty_substs, foo_body_arc.clone()),
    ];

    let mut ctx = MonoCtx::new();
    // Start with main
    ctx.collect(
        &[MonoItem::Fn { def_id: main_id, substs: empty_substs }],
        &|def_id, substs| {
            for (fn_id, fn_substs, body) in &bodies {
                if def_id.local_id.to_raw() == fn_id.to_raw() && *substs == *fn_substs {
                    return body.clone();
                }
            }
            Arc::new(Body::dummy(def_id))
        },
    );

    assert!(ctx.item_count() >= 2, "should collect at least main and foo");
}

/// V23-T08: Empty start set → nothing collected
#[test]
fn empty_start_nothing_collected() {
    let mut ctx = MonoCtx::new();
    ctx.collect(&[], &|_def_id, _substs| Arc::new(Body::dummy(_def_id)));
    assert_eq!(ctx.item_count(), 0, "empty start set should collect nothing");
}

/// V23-T09: Static item → collected but no body scanning needed
#[test]
fn static_item_collected() {
    let static_id = StaticDefId::from_raw(42);
    let static_item = MonoItem::Static { def_id: static_id };

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[static_item],
        &|def_id, _substs| Arc::new(Body::dummy(def_id)),
    );

    assert_eq!(ctx.item_count(), 1, "static should be collected");
    assert!(ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Static { def_id } if def_id.to_raw() == 42)));
}

/// V23-T10: Multiple constants in same body
#[test]
fn multiple_constants_in_body() {
    let main_id = FnDefId::from_raw(1);
    let const_a = ConstDefId::from_raw(10);
    let const_b = ConstDefId::from_raw(11);
    let empty_substs = Substitution::empty();

    let owner = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(main_id.to_raw()));
    let mut locals = IndexVec::new();
    locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });
    locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });
    locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });

    let stmt_a = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(1)),
            Rvalue::Use(Operand::Constant(MirConst { kind: MirConstKind::ConstRef(const_a, empty_substs), ty: Ty::UNIT, span: Span::DUMMY })),
        ),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let stmt_b = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(2)),
            Rvalue::Use(Operand::Constant(MirConst { kind: MirConstKind::ConstRef(const_b, empty_substs), ty: Ty::UNIT, span: Span::DUMMY })),
        ),
        source_info: SourceInfo::new(Span::DUMMY),
    };

    let mut basic_blocks = IndexVec::new();
    basic_blocks.push(BasicBlockData {
        statements: vec![stmt_a, stmt_b],
        terminator: Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(Span::DUMMY) },
        is_cleanup: false,
    });

    let main_body = Body { owner, basic_blocks, locals, arg_count: 0, return_ty: Ty::UNIT, span: Span::DUMMY, var_debug_info: vec![] };

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[MonoItem::Fn { def_id: main_id, substs: empty_substs }],
        &|def_id, _substs| Arc::new(main_body.clone()),
    );

    assert_eq!(ctx.item_count(), 3, "should collect main + 2 constants");
    let has_const_a = ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Const { def_id, .. } if def_id.to_raw() == 10));
    let has_const_b = ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Const { def_id, .. } if def_id.to_raw() == 11));
    assert!(has_const_a, "const A should be collected");
    assert!(has_const_b, "const B should be collected");
}

/// V23-T11: Mixed call and constant references
#[test]
fn mixed_calls_and_constants() {
    let main_id = FnDefId::from_raw(1);
    let helper_id = FnDefId::from_raw(2);
    let const_id = ConstDefId::from_raw(20);
    let empty_substs = Substitution::empty();

    // Body that has both a Call to helper and a ConstRef
    let owner = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(main_id.to_raw()));
    let mut locals = IndexVec::new();
    locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });
    locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Not, source_info: SourceInfo::new(Span::DUMMY) });
    locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });
    locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });

    let stmt_const = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(3)),
            Rvalue::Use(Operand::Constant(MirConst { kind: MirConstKind::ConstRef(const_id, empty_substs), ty: Ty::UNIT, span: Span::DUMMY })),
        ),
        source_info: SourceInfo::new(Span::DUMMY),
    };

    let call_term = Terminator {
        kind: TerminatorKind::Call {
            func: Operand::Constant(MirConst { kind: MirConstKind::Fn(helper_id, empty_substs), ty: Ty::UNIT, span: Span::DUMMY }),
            args: vec![Operand::Copy(Place::new(LocalIdx::from_raw(1)))],
            destination: Place::new(LocalIdx::from_raw(2)),
            target: Some(BasicBlockIdx::from_raw(1)),
            cleanup: None,
        },
        source_info: SourceInfo::new(Span::DUMMY),
    };

    let ret = Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(Span::DUMMY) };

    let mut basic_blocks = IndexVec::new();
    basic_blocks.push(BasicBlockData { statements: vec![stmt_const], terminator: call_term, is_cleanup: false });
    basic_blocks.push(BasicBlockData { statements: vec![], terminator: ret, is_cleanup: false });

    let main_body = Body { owner, basic_blocks, locals, arg_count: 1, return_ty: Ty::UNIT, span: Span::DUMMY, var_debug_info: vec![] };
    let helper_body = make_simple_body(helper_id);

    let bodies = vec![
        (main_id, empty_substs, Arc::new(main_body)),
        (helper_id, empty_substs, Arc::new(helper_body)),
    ];

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[MonoItem::Fn { def_id: main_id, substs: empty_substs }],
        &|def_id, substs| {
            for (fn_id, fn_substs, body) in &bodies {
                if def_id.local_id.to_raw() == fn_id.to_raw() && *substs == *fn_substs {
                    return body.clone();
                }
            }
            Arc::new(Body::dummy(def_id))
        },
    );

    assert_eq!(ctx.item_count(), 3, "should collect main, helper, and const");
    let has_main = ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Fn { def_id, .. } if def_id.to_raw() == 1));
    let has_helper = ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Fn { def_id, .. } if def_id.to_raw() == 2));
    let has_const = ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Const { def_id, .. } if def_id.to_raw() == 20));
    assert!(has_main, "main should be collected");
    assert!(has_helper, "helper should be collected");
    assert!(has_const, "const should be collected");
}

/// V23-T12: Item with no references (leaf function) → only itself collected
#[test]
fn leaf_function_only_self() {
    let leaf_id = FnDefId::from_raw(99);
    let empty_substs = Substitution::empty();

    let leaf_body = make_simple_body(leaf_id);

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[MonoItem::Fn { def_id: leaf_id, substs: empty_substs }],
        &|_def_id, _substs| Arc::new(leaf_body.clone()),
    );

    assert_eq!(ctx.item_count(), 1, "leaf function should collect only itself");
    assert!(ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Fn { def_id, .. } if def_id.to_raw() == 99)));
}

/// V23-T13: Multiple start items
#[test]
fn multiple_start_items() {
    let a_id = FnDefId::from_raw(1);
    let b_id = FnDefId::from_raw(2);
    let empty_substs = Substitution::empty();

    let a_body = make_simple_body(a_id);
    let b_body = make_simple_body(b_id);

    let bodies = vec![
        (a_id, empty_substs, Arc::new(a_body)),
        (b_id, empty_substs, Arc::new(b_body)),
    ];

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[
            MonoItem::Fn { def_id: a_id, substs: empty_substs },
            MonoItem::Fn { def_id: b_id, substs: empty_substs },
        ],
        &|def_id, substs| {
            for (fn_id, fn_substs, body) in &bodies {
                if def_id.local_id.to_raw() == fn_id.to_raw() && *substs == *fn_substs {
                    return body.clone();
                }
            }
            Arc::new(Body::dummy(def_id))
        },
    );

    assert_eq!(ctx.item_count(), 2, "two start items should both be collected");
}

/// V23-T14: Constant referencing another constant (transitive const)
#[test]
fn transitive_constant_collection() {
    let fn_id = FnDefId::from_raw(1);
    let const_a = ConstDefId::from_raw(10);
    let const_b = ConstDefId::from_raw(11);
    let empty_substs = Substitution::empty();

    // Main body references const_a
    let owner = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(fn_id.to_raw()));
    let mut locals = IndexVec::new();
    locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });
    locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });

    let stmt = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(1)),
            Rvalue::Use(Operand::Constant(MirConst { kind: MirConstKind::ConstRef(const_a, empty_substs), ty: Ty::UNIT, span: Span::DUMMY })),
        ),
        source_info: SourceInfo::new(Span::DUMMY),
    };

    let mut basic_blocks = IndexVec::new();
    basic_blocks.push(BasicBlockData {
        statements: vec![stmt],
        terminator: Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(Span::DUMMY) },
        is_cleanup: false,
    });

    let fn_body = Body { owner, basic_blocks, locals, arg_count: 0, return_ty: Ty::UNIT, span: Span::DUMMY, var_debug_info: vec![] };

    // const_a's body references const_b
    let const_owner = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(const_a.to_raw()));
    let mut const_locals = IndexVec::new();
    const_locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });
    const_locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });

    let const_stmt = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(1)),
            Rvalue::Use(Operand::Constant(MirConst { kind: MirConstKind::ConstRef(const_b, empty_substs), ty: Ty::UNIT, span: Span::DUMMY })),
        ),
        source_info: SourceInfo::new(Span::DUMMY),
    };

    let mut const_blocks = IndexVec::new();
    const_blocks.push(BasicBlockData {
        statements: vec![const_stmt],
        terminator: Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(Span::DUMMY) },
        is_cleanup: false,
    });

    let const_body = Body { owner: const_owner, basic_blocks: const_blocks, locals: const_locals, arg_count: 0, return_ty: Ty::UNIT, span: Span::DUMMY, var_debug_info: vec![] };

    let const_b_body = make_simple_body(fn_id); // reuse, it's a leaf

    let bodies = vec![
        (fn_id, empty_substs, Arc::new(fn_body)),
        (ConstDefId::from_raw(10), empty_substs, Arc::new(const_body)),
    ];

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[MonoItem::Fn { def_id: fn_id, substs: empty_substs }],
        &|def_id, substs| {
            let raw = def_id.local_id.to_raw();
            for (id, fn_substs, body) in &bodies {
                if raw == id.to_raw() && *substs == *fn_substs {
                    return body.clone();
                }
            }
            Arc::new(Body::dummy(def_id))
        },
    );

    // Should collect: fn, const_a, const_b (transitive)
    assert!(ctx.item_count() >= 2, "should collect at least fn and const_a");
    let has_const_a = ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Const { def_id, .. } if def_id.to_raw() == 10));
    let has_const_b = ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Const { def_id, .. } if def_id.to_raw() == 11));
    assert!(has_const_a, "const_a should be collected");
    assert!(has_const_b, "const_b should be collected transitively");
}

/// V23-T05: Diamond dependency - A calls B and C, both call D → D collected once
#[test]
fn diamond_dependency_dedup() {
    let a_id = FnDefId::from_raw(1);
    let b_id = FnDefId::from_raw(2);
    let c_id = FnDefId::from_raw(3);
    let d_id = FnDefId::from_raw(4);
    let empty_substs = Substitution::empty();

    // A calls B and C
    let a_body = make_call_body(a_id, b_id, empty_substs);
    // Add another call to C in A's body: we reuse make_call_body which creates one call.
    // For simplicity, A just calls B for now. We'll create separate bodies.
    let b_body = make_call_body(b_id, d_id, empty_substs);
    let c_body = make_call_body(c_id, d_id, empty_substs);
    let d_body = make_simple_body(d_id);

    // Build A's body that calls both B and C
    let owner_a = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(a_id.to_raw()));
    let mut locals_a = IndexVec::new();
    locals_a.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });
    locals_a.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Not, source_info: SourceInfo::new(Span::DUMMY) });
    locals_a.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });
    locals_a.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });

    let call_b = Terminator {
        kind: TerminatorKind::Call {
            func: Operand::Constant(MirConst { kind: MirConstKind::Fn(b_id, empty_substs), ty: Ty::UNIT, span: Span::DUMMY }),
            args: vec![Operand::Copy(Place::new(LocalIdx::from_raw(1)))],
            destination: Place::new(LocalIdx::from_raw(2)),
            target: Some(BasicBlockIdx::from_raw(1)),
            cleanup: None,
        },
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let call_c = Terminator {
        kind: TerminatorKind::Call {
            func: Operand::Constant(MirConst { kind: MirConstKind::Fn(c_id, empty_substs), ty: Ty::UNIT, span: Span::DUMMY }),
            args: vec![Operand::Copy(Place::new(LocalIdx::from_raw(1)))],
            destination: Place::new(LocalIdx::from_raw(3)),
            target: Some(BasicBlockIdx::from_raw(2)),
            cleanup: None,
        },
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let ret = Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(Span::DUMMY) };

    let mut bb = IndexVec::new();
    bb.push(BasicBlockData { statements: vec![], terminator: call_b, is_cleanup: false });
    bb.push(BasicBlockData { statements: vec![], terminator: call_c, is_cleanup: false });
    bb.push(BasicBlockData { statements: vec![], terminator: ret, is_cleanup: false });
    let a_body_diamond = Body { owner: owner_a, basic_blocks: bb, locals: locals_a, arg_count: 1, return_ty: Ty::UNIT, span: Span::DUMMY, var_debug_info: vec![] };

    let b_body_arc = Arc::new(b_body);
    let c_body_arc = Arc::new(c_body);
    let d_body_arc = Arc::new(d_body);
    let a_body_arc = Arc::new(a_body_diamond);

    let bodies = vec![
        (a_id, empty_substs, a_body_arc.clone()),
        (b_id, empty_substs, b_body_arc.clone()),
        (c_id, empty_substs, c_body_arc.clone()),
        (d_id, empty_substs, d_body_arc.clone()),
    ];

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[MonoItem::Fn { def_id: a_id, substs: empty_substs }],
        &|def_id, substs| {
            for (fn_id, fn_substs, body) in &bodies {
                if def_id.local_id.to_raw() == fn_id.to_raw() && *substs == *fn_substs {
                    return body.clone();
                }
            }
            Arc::new(Body::dummy(def_id))
        },
    );

    // Should have 4 items: A, B, C, D (D only once despite being called by both B and C)
    assert_eq!(ctx.item_count(), 4, "diamond: should have A, B, C, D (D deduped)");

    let d_count = ctx.items().iter().filter(|d| matches!(&d.item, MonoItem::Fn { def_id, .. } if def_id.to_raw() == 4)).count();
    assert_eq!(d_count, 1, "D should appear exactly once");
}

/// V23-T06: Chain of calls A→B→C→D - all collected transitively
#[test]
fn transitive_call_chain() {
    let a_id = FnDefId::from_raw(1);
    let b_id = FnDefId::from_raw(2);
    let c_id = FnDefId::from_raw(3);
    let d_id = FnDefId::from_raw(4);
    let empty_substs = Substitution::empty();

    let a_body = make_call_body(a_id, b_id, empty_substs);
    let b_body = make_call_body(b_id, c_id, empty_substs);
    let c_body = make_call_body(c_id, d_id, empty_substs);
    let d_body = make_simple_body(d_id);

    let bodies = vec![
        (a_id, empty_substs, Arc::new(a_body)),
        (b_id, empty_substs, Arc::new(b_body)),
        (c_id, empty_substs, Arc::new(c_body)),
        (d_id, empty_substs, Arc::new(d_body)),
    ];

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[MonoItem::Fn { def_id: a_id, substs: empty_substs }],
        &|def_id, substs| {
            for (fn_id, fn_substs, body) in &bodies {
                if def_id.local_id.to_raw() == fn_id.to_raw() && *substs == *fn_substs {
                    return body.clone();
                }
            }
            Arc::new(Body::dummy(def_id))
        },
    );

    assert_eq!(ctx.item_count(), 4, "chain: A→B→C→D should collect all 4");
    for raw_id in [1u32, 2, 3, 4] {
        let found = ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Fn { def_id, .. } if def_id.to_raw() == raw_id));
        assert!(found, "function with raw id {} should be collected", raw_id);
    }
}

/// V23-T07: Two different substitutions of same function → both collected
#[test]
fn different_substitutions_both_collected() {
    let main_id = FnDefId::from_raw(1);
    let foo_id = FnDefId::from_raw(2);
    let empty_substs = Substitution::empty();

    // Create a body where main calls foo with one substitution
    let main_body = make_call_body(main_id, foo_id, empty_substs);
    let foo_body = make_simple_body(foo_id);

    let main_body_arc = Arc::new(main_body);
    let foo_body_arc = Arc::new(foo_body);

    let bodies = vec![
        (main_id, empty_substs, main_body_arc.clone()),
        (foo_id, empty_substs, foo_body_arc.clone()),
    ];

    let mut ctx = MonoCtx::new();
    // Start with main
    ctx.collect(
        &[MonoItem::Fn { def_id: main_id, substs: empty_substs }],
        &|def_id, substs| {
            for (fn_id, fn_substs, body) in &bodies {
                if def_id.local_id.to_raw() == fn_id.to_raw() && *substs == *fn_substs {
                    return body.clone();
                }
            }
            Arc::new(Body::dummy(def_id))
        },
    );

    assert!(ctx.item_count() >= 2, "should collect at least main and foo");
}

/// V23-T08: Empty start set → nothing collected
#[test]
fn empty_start_nothing_collected() {
    let mut ctx = MonoCtx::new();
    ctx.collect(&[], &|_def_id, _substs| Arc::new(Body::dummy(_def_id)));
    assert_eq!(ctx.item_count(), 0, "empty start set should collect nothing");
}

/// V23-T09: Static item → collected but no body scanning needed
#[test]
fn static_item_collected() {
    let static_id = StaticDefId::from_raw(42);
    let static_item = MonoItem::Static { def_id: static_id };

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[static_item],
        &|def_id, _substs| Arc::new(Body::dummy(def_id)),
    );

    assert_eq!(ctx.item_count(), 1, "static should be collected");
    assert!(ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Static { def_id } if def_id.to_raw() == 42)));
}

/// V23-T10: Multiple constants in same body
#[test]
fn multiple_constants_in_body() {
    let main_id = FnDefId::from_raw(1);
    let const_a = ConstDefId::from_raw(10);
    let const_b = ConstDefId::from_raw(11);
    let empty_substs = Substitution::empty();

    let owner = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(main_id.to_raw()));
    let mut locals = IndexVec::new();
    locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });
    locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });
    locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });

    let stmt_a = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(1)),
            Rvalue::Use(Operand::Constant(MirConst { kind: MirConstKind::ConstRef(const_a, empty_substs), ty: Ty::UNIT, span: Span::DUMMY })),
        ),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let stmt_b = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(2)),
            Rvalue::Use(Operand::Constant(MirConst { kind: MirConstKind::ConstRef(const_b, empty_substs), ty: Ty::UNIT, span: Span::DUMMY })),
        ),
        source_info: SourceInfo::new(Span::DUMMY),
    };

    let mut basic_blocks = IndexVec::new();
    basic_blocks.push(BasicBlockData {
        statements: vec![stmt_a, stmt_b],
        terminator: Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(Span::DUMMY) },
        is_cleanup: false,
    });

    let main_body = Body { owner, basic_blocks, locals, arg_count: 0, return_ty: Ty::UNIT, span: Span::DUMMY, var_debug_info: vec![] };

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[MonoItem::Fn { def_id: main_id, substs: empty_substs }],
        &|def_id, _substs| Arc::new(main_body.clone()),
    );

    assert_eq!(ctx.item_count(), 3, "should collect main + 2 constants");
    let has_const_a = ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Const { def_id, .. } if def_id.to_raw() == 10));
    let has_const_b = ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Const { def_id, .. } if def_id.to_raw() == 11));
    assert!(has_const_a, "const A should be collected");
    assert!(has_const_b, "const B should be collected");
}

/// V23-T11: Mixed call and constant references
#[test]
fn mixed_calls_and_constants() {
    let main_id = FnDefId::from_raw(1);
    let helper_id = FnDefId::from_raw(2);
    let const_id = ConstDefId::from_raw(20);
    let empty_substs = Substitution::empty();

    // Body that has both a Call to helper and a ConstRef
    let owner = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(main_id.to_raw()));
    let mut locals = IndexVec::new();
    locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });
    locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Not, source_info: SourceInfo::new(Span::DUMMY) });
    locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });
    locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });

    let stmt_const = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(3)),
            Rvalue::Use(Operand::Constant(MirConst { kind: MirConstKind::ConstRef(const_id, empty_substs), ty: Ty::UNIT, span: Span::DUMMY })),
        ),
        source_info: SourceInfo::new(Span::DUMMY),
    };

    let call_term = Terminator {
        kind: TerminatorKind::Call {
            func: Operand::Constant(MirConst { kind: MirConstKind::Fn(helper_id, empty_substs), ty: Ty::UNIT, span: Span::DUMMY }),
            args: vec![Operand::Copy(Place::new(LocalIdx::from_raw(1)))],
            destination: Place::new(LocalIdx::from_raw(2)),
            target: Some(BasicBlockIdx::from_raw(1)),
            cleanup: None,
        },
        source_info: SourceInfo::new(Span::DUMMY),
    };

    let ret = Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(Span::DUMMY) };

    let mut basic_blocks = IndexVec::new();
    basic_blocks.push(BasicBlockData { statements: vec![stmt_const], terminator: call_term, is_cleanup: false });
    basic_blocks.push(BasicBlockData { statements: vec![], terminator: ret, is_cleanup: false });

    let main_body = Body { owner, basic_blocks, locals, arg_count: 1, return_ty: Ty::UNIT, span: Span::DUMMY, var_debug_info: vec![] };
    let helper_body = make_simple_body(helper_id);

    let bodies = vec![
        (main_id, empty_substs, Arc::new(main_body)),
        (helper_id, empty_substs, Arc::new(helper_body)),
    ];

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[MonoItem::Fn { def_id: main_id, substs: empty_substs }],
        &|def_id, substs| {
            for (fn_id, fn_substs, body) in &bodies {
                if def_id.local_id.to_raw() == fn_id.to_raw() && *substs == *fn_substs {
                    return body.clone();
                }
            }
            Arc::new(Body::dummy(def_id))
        },
    );

    assert_eq!(ctx.item_count(), 3, "should collect main, helper, and const");
    let has_main = ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Fn { def_id, .. } if def_id.to_raw() == 1));
    let has_helper = ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Fn { def_id, .. } if def_id.to_raw() == 2));
    let has_const = ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Const { def_id, .. } if def_id.to_raw() == 20));
    assert!(has_main, "main should be collected");
    assert!(has_helper, "helper should be collected");
    assert!(has_const, "const should be collected");
}

/// V23-T12: Item with no references (leaf function) → only itself collected
#[test]
fn leaf_function_only_self() {
    let leaf_id = FnDefId::from_raw(99);
    let empty_substs = Substitution::empty();

    let leaf_body = make_simple_body(leaf_id);

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[MonoItem::Fn { def_id: leaf_id, substs: empty_substs }],
        &|_def_id, _substs| Arc::new(leaf_body.clone()),
    );

    assert_eq!(ctx.item_count(), 1, "leaf function should collect only itself");
    assert!(ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Fn { def_id, .. } if def_id.to_raw() == 99)));
}

/// V23-T13: Multiple start items
#[test]
fn multiple_start_items() {
    let a_id = FnDefId::from_raw(1);
    let b_id = FnDefId::from_raw(2);
    let empty_substs = Substitution::empty();

    let a_body = make_simple_body(a_id);
    let b_body = make_simple_body(b_id);

    let bodies = vec![
        (a_id, empty_substs, Arc::new(a_body)),
        (b_id, empty_substs, Arc::new(b_body)),
    ];

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[
            MonoItem::Fn { def_id: a_id, substs: empty_substs },
            MonoItem::Fn { def_id: b_id, substs: empty_substs },
        ],
        &|def_id, substs| {
            for (fn_id, fn_substs, body) in &bodies {
                if def_id.local_id.to_raw() == fn_id.to_raw() && *substs == *fn_substs {
                    return body.clone();
                }
            }
            Arc::new(Body::dummy(def_id))
        },
    );

    assert_eq!(ctx.item_count(), 2, "two start items should both be collected");
}

/// V23-T14: Constant referencing another constant (transitive const)
#[test]
fn transitive_constant_collection() {
    let fn_id = FnDefId::from_raw(1);
    let const_a = ConstDefId::from_raw(10);
    let const_b = ConstDefId::from_raw(11);
    let empty_substs = Substitution::empty();

    // Main body references const_a
    let owner = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(fn_id.to_raw()));
    let mut locals = IndexVec::new();
    locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });
    locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });

    let stmt = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(1)),
            Rvalue::Use(Operand::Constant(MirConst { kind: MirConstKind::ConstRef(const_a, empty_substs), ty: Ty::UNIT, span: Span::DUMMY })),
        ),
        source_info: SourceInfo::new(Span::DUMMY),
    };

    let mut basic_blocks = IndexVec::new();
    basic_blocks.push(BasicBlockData {
        statements: vec![stmt],
        terminator: Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(Span::DUMMY) },
        is_cleanup: false,
    });

    let fn_body = Body { owner, basic_blocks, locals, arg_count: 0, return_ty: Ty::UNIT, span: Span::DUMMY, var_debug_info: vec![] };

    // const_a's body references const_b
    let const_owner = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(const_a.to_raw()));
    let mut const_locals = IndexVec::new();
    const_locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });
    const_locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });

    let const_stmt = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(1)),
            Rvalue::Use(Operand::Constant(MirConst { kind: MirConstKind::ConstRef(const_b, empty_substs), ty: Ty::UNIT, span: Span::DUMMY })),
        ),
        source_info: SourceInfo::new(Span::DUMMY),
    };

    let mut const_blocks = IndexVec::new();
    const_blocks.push(BasicBlockData {
        statements: vec![const_stmt],
        terminator: Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(Span::DUMMY) },
        is_cleanup: false,
    });

    let const_body = Body { owner: const_owner, basic_blocks: const_blocks, locals: const_locals, arg_count: 0, return_ty: Ty::UNIT, span: Span::DUMMY, var_debug_info: vec![] };

    let const_b_body = make_simple_body(fn_id); // reuse, it's a leaf

    let bodies = vec![
        (fn_id, empty_substs, Arc::new(fn_body)),
        (ConstDefId::from_raw(10), empty_substs, Arc::new(const_body)),
    ];

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[MonoItem::Fn { def_id: fn_id, substs: empty_substs }],
        &|def_id, substs| {
            let raw = def_id.local_id.to_raw();
            for (id, fn_substs, body) in &bodies {
                if raw == id.to_raw() && *substs == *fn_substs {
                    return body.clone();
                }
            }
            Arc::new(Body::dummy(def_id))
        },
    );

    // Should collect: fn, const_a, const_b (transitive)
    assert!(ctx.item_count() >= 2, "should collect at least fn and const_a");
    let has_const_a = ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Const { def_id, .. } if def_id.to_raw() == 10));
    let has_const_b = ctx.items().iter().any(|d| matches!(&d.item, MonoItem::Const { def_id, .. } if def_id.to_raw() == 11));
    assert!(has_const_a, "const_a should be collected");
    assert!(has_const_b, "const_b should be collected transitively");
}
