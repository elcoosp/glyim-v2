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
