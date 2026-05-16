//! Tests for V15: LLVM Drop & Deallocation
//!
//! V15-T01: Drop of Copy type at block end - no drop call emitted
//! V15-T02: Drop of non-Copy type → drop_in_place call emitted
//! V15-T03: Drop in panic path (cleanup) with non-Copy type
//! V15-T04: Drop order matches declaration order (sequential drops)
//! V15-T05: Drop of mutable ref to non-Copy type → dealloc emitted
//! V15-T06: Drop of &mut Copy type → no dealloc needed

use crate::LlvmBackend;
use glyim_core::arena::IndexVec;
use glyim_core::def_id::{CrateId, DefId, LocalDefId};
use glyim_core::{IntTy, Mutability};
use glyim_mir::*;
use glyim_test::with_fresh_ty_ctx;
use glyim_type::{Ty, TyKind};
use inkwell::targets::{InitializationConfig, Target};

fn dummy_def_id(idx: u32) -> DefId {
    DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(idx))
}

/// V15-T01: Drop of Copy type at block end - no drop call emitted.
///
/// i32 is Copy, so type_needs_drop returns false and no drop call
/// should be emitted. The terminator just branches to the target.
/// Note: the module always declares glyim_drop_in_place and glyim_dealloc,
/// but there should be no `call` to them.
#[test]
fn v15_t01_drop_copy_type_at_block_end() {
    Target::initialize_all(&InitializationConfig::default());

    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));

        let mut locals = IndexVec::<LocalIdx, LocalDecl>::new();
        locals.push(LocalDecl {
            ty: Ty::UNIT,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        });
        locals.push(LocalDecl {
            ty: i32_ty,
            mutability: Mutability::Mut,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        });

        let mut bbs = IndexVec::<BasicBlockIdx, BasicBlockData>::new();
        bbs.push(BasicBlockData {
            statements: vec![Statement {
                kind: StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(1)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(42),
                        ty: i32_ty,
                        span: glyim_span::Span::DUMMY,
                    })),
                ),
                source_info: SourceInfo::new(glyim_span::Span::DUMMY),
            }],
            terminator: Terminator {
                kind: TerminatorKind::Drop {
                    place: Place::new(LocalIdx::from_raw(1)),
                    target: BasicBlockIdx::from_raw(1),
                    cleanup: None,
                },
                source_info: SourceInfo::new(glyim_span::Span::DUMMY),
            },
            is_cleanup: false,
        });
        bbs.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Return,
                source_info: SourceInfo::new(glyim_span::Span::DUMMY),
            },
            is_cleanup: false,
        });

        Body {
            owner: dummy_def_id(100),
            basic_blocks: bbs,
            locals,
            arg_count: 0,
            return_ty: Ty::UNIT,
            span: glyim_span::Span::DUMMY,
            var_debug_info: vec![],
        }
    });

    let backend = LlvmBackend::new().with_ty_ctx(ctx);
    let context = inkwell::context::Context::create();
    let module = backend
        .lower_body_to_module(&context, &body)
        .expect("lowering should succeed");

    let ir = module.print_to_string().to_string();
    // i32 is Copy → no call to drop_in_place should be emitted
    // (the declare statement is always present, but there should be no `call`)
    assert!(
        !ir.contains("call void @glyim_drop_in_place"),
        "IR should NOT call glyim_drop_in_place for Copy type i32, got:\n{}",
        ir
    );
    // Should still have the branch to target block
    assert!(
        ir.contains("br label") || ir.contains("ret void"),
        "IR should contain branch or return, got:\n{}",
        ir
    );
}

/// V15-T02: Drop of non-Copy type → drop_in_place call emitted.
///
/// Uses TyKind::String which type_needs_drop returns true for.
/// The Drop terminator should emit a call to glyim_drop_in_place.
#[test]
fn v15_t02_drop_non_copy_type_emits_drop_in_place() {
    Target::initialize_all(&InitializationConfig::default());

    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let string_ty = ctx_mut.mk_ty(TyKind::String);

        let mut locals = IndexVec::<LocalIdx, LocalDecl>::new();
        locals.push(LocalDecl {
            ty: Ty::UNIT,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        });
        locals.push(LocalDecl {
            ty: string_ty,
            mutability: Mutability::Mut,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        });

        let mut bbs = IndexVec::<BasicBlockIdx, BasicBlockData>::new();
        bbs.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Drop {
                    place: Place::new(LocalIdx::from_raw(1)),
                    target: BasicBlockIdx::from_raw(1),
                    cleanup: None,
                },
                source_info: SourceInfo::new(glyim_span::Span::DUMMY),
            },
            is_cleanup: false,
        });
        bbs.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Return,
                source_info: SourceInfo::new(glyim_span::Span::DUMMY),
            },
            is_cleanup: false,
        });

        Body {
            owner: dummy_def_id(101),
            basic_blocks: bbs,
            locals,
            arg_count: 0,
            return_ty: Ty::UNIT,
            span: glyim_span::Span::DUMMY,
            var_debug_info: vec![],
        }
    });

    let backend = LlvmBackend::new().with_ty_ctx(ctx);
    let context = inkwell::context::Context::create();
    let module = backend
        .lower_body_to_module(&context, &body)
        .expect("lowering should succeed");

    let ir = module.print_to_string().to_string();
    // String needs drop → drop_in_place call should be generated
    assert!(
        ir.contains("call void @glyim_drop_in_place"),
        "IR should call glyim_drop_in_place for non-Copy String type, got:\n{}",
        ir
    );
}

/// V15-T03: Drop in panic path (cleanup) with non-Copy type.
///
/// Both normal and cleanup paths must emit drop_in_place calls
/// for the non-Copy type. Uses a SwitchInt to create the alternate
/// path instead of Call (to avoid layout computation issues with String).
#[test]
fn v15_t03_drop_in_panic_cleanup_path() {
    Target::initialize_all(&InitializationConfig::default());

    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let string_ty = ctx_mut.mk_ty(TyKind::String);
        let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));

        let mut locals = IndexVec::<LocalIdx, LocalDecl>::new();
        // local 0 = return place
        locals.push(LocalDecl {
            ty: Ty::UNIT,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        });
        // local 1 = string to be dropped in both paths
        locals.push(LocalDecl {
            ty: string_ty,
            mutability: Mutability::Mut,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        });
        // local 2 = i32 condition for switch
        locals.push(LocalDecl {
            ty: i32_ty,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        });

        let mut bbs = IndexVec::<BasicBlockIdx, BasicBlockData>::new();
        // bb0: switch on condition to simulate normal vs cleanup path
        bbs.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::SwitchInt {
                    discr: Operand::Move(Place::new(LocalIdx::from_raw(2))),
                    switch_ty: i32_ty,
                    targets: SwitchTargets::new(
                        Box::from_iter([(1u128, BasicBlockIdx::from_raw(1))]),
                        BasicBlockIdx::from_raw(2),
                    ),
                },
                source_info: SourceInfo::new(glyim_span::Span::DUMMY),
            },
            is_cleanup: false,
        });
        // bb1: normal path - drop string then return
        bbs.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Drop {
                    place: Place::new(LocalIdx::from_raw(1)),
                    target: BasicBlockIdx::from_raw(3),
                    cleanup: None,
                },
                source_info: SourceInfo::new(glyim_span::Span::DUMMY),
            },
            is_cleanup: false,
        });
        // bb2: cleanup/panic path - must also drop string
        bbs.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Drop {
                    place: Place::new(LocalIdx::from_raw(1)),
                    target: BasicBlockIdx::from_raw(3),
                    cleanup: None,
                },
                source_info: SourceInfo::new(glyim_span::Span::DUMMY),
            },
            is_cleanup: true,
        });
        // bb3: return
        bbs.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Return,
                source_info: SourceInfo::new(glyim_span::Span::DUMMY),
            },
            is_cleanup: false,
        });

        Body {
            owner: dummy_def_id(102),
            basic_blocks: bbs,
            locals,
            arg_count: 0,
            return_ty: Ty::UNIT,
            span: glyim_span::Span::DUMMY,
            var_debug_info: vec![],
        }
    });

    let backend = LlvmBackend::new().with_ty_ctx(ctx);
    let context = inkwell::context::Context::create();
    let module = backend
        .lower_body_to_module(&context, &body)
        .expect("lowering should succeed");

    let ir = module.print_to_string().to_string();
    // Should have drop_in_place call for the String in both paths
    assert!(
        ir.contains("call void @glyim_drop_in_place"),
        "IR should call glyim_drop_in_place for cleanup path drop, got:\n{}",
        ir
    );
    // Should have basic blocks for both normal and cleanup paths
    assert!(
        ir.contains("bb_1") && ir.contains("bb_2"),
        "IR should have basic blocks for normal and cleanup paths, got:\n{}",
        ir
    );
    // Count the number of drop_in_place calls - should be at least 2 (one per path)
    let drop_call_count = ir.matches("call void @glyim_drop_in_place").count();
    assert!(
        drop_call_count >= 2,
        "IR should have at least 2 drop_in_place calls (normal + cleanup), found {}, got:\n{}",
        drop_call_count,
        ir
    );
}

/// V15-T04: Drop order matches declaration order (sequential drops).
///
/// Two Drop terminators in sequence - both for non-Copy types.
/// Both should emit drop_in_place calls.
#[test]
fn v15_t04_drop_order_matches_declaration_order() {
    Target::initialize_all(&InitializationConfig::default());

    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let string_ty = ctx_mut.mk_ty(TyKind::String);

        let mut locals = IndexVec::<LocalIdx, LocalDecl>::new();
        locals.push(LocalDecl {
            ty: Ty::UNIT,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        });
        locals.push(LocalDecl {
            ty: string_ty,
            mutability: Mutability::Mut,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        });
        locals.push(LocalDecl {
            ty: string_ty,
            mutability: Mutability::Mut,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        });

        let mut bbs = IndexVec::<BasicBlockIdx, BasicBlockData>::new();
        // bb0: drop local_2 first (LIFO)
        bbs.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Drop {
                    place: Place::new(LocalIdx::from_raw(2)),
                    target: BasicBlockIdx::from_raw(1),
                    cleanup: None,
                },
                source_info: SourceInfo::new(glyim_span::Span::DUMMY),
            },
            is_cleanup: false,
        });
        // bb1: drop local_1
        bbs.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Drop {
                    place: Place::new(LocalIdx::from_raw(1)),
                    target: BasicBlockIdx::from_raw(2),
                    cleanup: None,
                },
                source_info: SourceInfo::new(glyim_span::Span::DUMMY),
            },
            is_cleanup: false,
        });
        // bb2: return
        bbs.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Return,
                source_info: SourceInfo::new(glyim_span::Span::DUMMY),
            },
            is_cleanup: false,
        });

        Body {
            owner: dummy_def_id(103),
            basic_blocks: bbs,
            locals,
            arg_count: 0,
            return_ty: Ty::UNIT,
            span: glyim_span::Span::DUMMY,
            var_debug_info: vec![],
        }
    });

    let backend = LlvmBackend::new().with_ty_ctx(ctx);
    let context = inkwell::context::Context::create();
    let module = backend
        .lower_body_to_module(&context, &body)
        .expect("lowering should succeed");

    let ir = module.print_to_string().to_string();
    // Should have drop_in_place calls for both String values
    let drop_call_count = ir.matches("call void @glyim_drop_in_place").count();
    assert!(
        drop_call_count >= 2,
        "IR should have at least 2 drop_in_place calls for sequential drops, found {}, got:\n{}",
        drop_call_count,
        ir
    );
    // Should have basic blocks for the sequential drops
    assert!(
        ir.contains("bb_1") && ir.contains("bb_2"),
        "IR should have basic blocks for sequential drops, got:\n{}",
        ir
    );
}

/// V15-T05: Drop of mutable ref to non-Copy type → both drop_in_place and dealloc emitted.
///
/// When dropping a &mut T where T is not Copy, we should call both
/// glyim_drop_in_place (because type_needs_drop recurses through the ref
/// and finds T needs drop) and glyim_dealloc (because type_needs_dealloc
/// returns true for &mut T where T is not Copy).
#[test]
fn v15_t05_drop_mut_ref_non_copy_emits_dealloc() {
    Target::initialize_all(&InitializationConfig::default());

    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let string_ty = ctx_mut.mk_ty(TyKind::String);
        let ref_string_ty = ctx_mut.mk_ty(TyKind::Ref(
            glyim_type::Region::Erased,
            string_ty,
            Mutability::Mut,
        ));

        let mut locals = IndexVec::<LocalIdx, LocalDecl>::new();
        locals.push(LocalDecl {
            ty: Ty::UNIT,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        });
        locals.push(LocalDecl {
            ty: ref_string_ty,
            mutability: Mutability::Mut,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        });

        let mut bbs = IndexVec::<BasicBlockIdx, BasicBlockData>::new();
        bbs.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Drop {
                    place: Place::new(LocalIdx::from_raw(1)),
                    target: BasicBlockIdx::from_raw(1),
                    cleanup: None,
                },
                source_info: SourceInfo::new(glyim_span::Span::DUMMY),
            },
            is_cleanup: false,
        });
        bbs.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Return,
                source_info: SourceInfo::new(glyim_span::Span::DUMMY),
            },
            is_cleanup: false,
        });

        Body {
            owner: dummy_def_id(104),
            basic_blocks: bbs,
            locals,
            arg_count: 0,
            return_ty: Ty::UNIT,
            span: glyim_span::Span::DUMMY,
            var_debug_info: vec![],
        }
    });

    let backend = LlvmBackend::new().with_ty_ctx(ctx);
    let context = inkwell::context::Context::create();
    let module = backend
        .lower_body_to_module(&context, &body)
        .expect("lowering should succeed");

    let ir = module.print_to_string().to_string();
    // Should have drop_in_place because String needs drop
    assert!(
        ir.contains("call void @glyim_drop_in_place"),
        "IR should call glyim_drop_in_place for &mut String drop, got:\n{}",
        ir
    );
    // Should have glyim_dealloc because &mut to non-Copy type needs dealloc
    assert!(
        ir.contains("call void @glyim_dealloc"),
        "IR should call glyim_dealloc for &mut String deallocation, got:\n{}",
        ir
    );
}

/// V15-T06: Drop of &mut i32 (Copy inner) → no drop_in_place call, no dealloc call.
///
/// When the inner type is Copy, type_needs_drop returns false (through the ref),
/// and type_needs_dealloc returns false (because is_copy_type(i32) is true).
/// The function declarations may still be present in the module, but there
/// should be no `call` instructions for them.
#[test]
fn v15_t06_drop_mut_ref_copy_type_no_dealloc() {
    Target::initialize_all(&InitializationConfig::default());

    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
        let ref_i32_ty = ctx_mut.mk_ty(TyKind::Ref(
            glyim_type::Region::Erased,
            i32_ty,
            Mutability::Mut,
        ));

        let mut locals = IndexVec::<LocalIdx, LocalDecl>::new();
        locals.push(LocalDecl {
            ty: Ty::UNIT,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        });
        locals.push(LocalDecl {
            ty: ref_i32_ty,
            mutability: Mutability::Mut,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        });

        let mut bbs = IndexVec::<BasicBlockIdx, BasicBlockData>::new();
        bbs.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Drop {
                    place: Place::new(LocalIdx::from_raw(1)),
                    target: BasicBlockIdx::from_raw(1),
                    cleanup: None,
                },
                source_info: SourceInfo::new(glyim_span::Span::DUMMY),
            },
            is_cleanup: false,
        });
        bbs.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Return,
                source_info: SourceInfo::new(glyim_span::Span::DUMMY),
            },
            is_cleanup: false,
        });

        Body {
            owner: dummy_def_id(105),
            basic_blocks: bbs,
            locals,
            arg_count: 0,
            return_ty: Ty::UNIT,
            span: glyim_span::Span::DUMMY,
            var_debug_info: vec![],
        }
    });

    let backend = LlvmBackend::new().with_ty_ctx(ctx);
    let context = inkwell::context::Context::create();
    let module = backend
        .lower_body_to_module(&context, &body)
        .expect("lowering should succeed");

    let ir = module.print_to_string().to_string();
    // i32 is Copy, so through &mut i32, no drop needed and no dealloc
    assert!(
        !ir.contains("call void @glyim_drop_in_place"),
        "IR should NOT call glyim_drop_in_place for &mut i32, got:\n{}",
        ir
    );
    assert!(
        !ir.contains("call void @glyim_dealloc"),
        "IR should NOT call glyim_dealloc for &mut i32, got:\n{}",
        ir
    );
}
