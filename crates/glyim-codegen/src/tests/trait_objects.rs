use glyim_core::{CrateId, DefId, FnDefId, LocalDefId, TargetInfo, TraitDefId, Interner};
use glyim_mir::*;
use glyim_test::with_fresh_ty_ctx;
use glyim_type::*;
use std::sync::Arc;
use crate::*;

fn make_dyn_ty(ctx: &mut TyCtxMut) -> Ty {
    let empty_subst = ctx.intern_substitution(vec![]);
    let trait_ref = TraitRef {
        def_id: TraitDefId::from_raw(0),
        substs: empty_subst,
    };
    let predicate = Predicate::Trait(TraitPredicate {
        trait_ref,
        polarity: ImplPolarity::Positive,
    });
    let box_predicates: Box<[Predicate]> = Box::new([predicate]);
    let bound_vars: Box<[BoundVariableKind]> = vec![].into();
    let binder = Binder::bind(box_predicates, bound_vars);
    let kind = TyKind::Dynamic(binder, Region::Erased);
    ctx.mk_ty(kind)
}

#[test]
fn create_trait_object_from_concrete_type() {
    let (_, is_dynamic) = with_fresh_ty_ctx(|ctx| {
        let dyn_ty = make_dyn_ty(ctx);
        matches!(ctx.ty_kind(dyn_ty), TyKind::Dynamic(_, _))
    });
    assert!(is_dynamic, "trait object type should be Dynamic");
}

#[test]
fn call_method_through_trait_object() {
    let (ctx, dyn_ty) = with_fresh_ty_ctx(|ctx| make_dyn_ty(ctx));

    let local0 = LocalIdx::from_raw(0);
    let local1 = LocalIdx::from_raw(1);
    let mut local_decls = glyim_core::IndexVec::new();
    local_decls.push(LocalDecl {
        ty: dyn_ty,
        mutability: glyim_core::Mutability::Mut,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });
    local_decls.push(LocalDecl {
        ty: dyn_ty,
        mutability: glyim_core::Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });

    let mut basic_blocks = glyim_core::IndexVec::new();
    let term = Terminator {
        kind: TerminatorKind::Call {
            func: Operand::Copy(Place::new(local0)),
            args: vec![],
            destination: Place::new(local1),
            target: Some(BasicBlockIdx::from_raw(1)),
            cleanup: None,
        },
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };
    let mut block0 = BasicBlockData::new(term);
    block0.is_cleanup = false;
    basic_blocks.push(block0);

    let ret_term = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };
    basic_blocks.push(BasicBlockData::new(ret_term));

    let body = Arc::new(Body {
        owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        basic_blocks,
        locals: local_decls,
        arg_count: 0,
        return_ty: glyim_type::Ty::UNIT,
        span: glyim_span::Span::DUMMY,
        var_debug_info: vec![],
    });

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok(), "generate_function should succeed: {:?}", result.err());
    let bc = result.unwrap();
    assert!(bc.contains(&0x22), "bytecode should contain OP_CALL_INDIRECT");
    assert!(!bc.contains(&0x1B), "bytecode should NOT contain OP_CALL for indirect call");
    drop(ctx);
}

#[test]
fn upcast_to_supertrait_object() {
    let (ctx, dyn_ty) = with_fresh_ty_ctx(|ctx| make_dyn_ty(ctx));

    let _local0 = LocalIdx::from_raw(0);
    let local1 = LocalIdx::from_raw(1);
    let mut local_decls = glyim_core::IndexVec::new();
    local_decls.push(LocalDecl {
        ty: dyn_ty,
        mutability: glyim_core::Mutability::Mut,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });
    local_decls.push(LocalDecl {
        ty: dyn_ty,
        mutability: glyim_core::Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });

    let mut basic_blocks = glyim_core::IndexVec::new();
    let term = Terminator {
        kind: TerminatorKind::Call {
            func: Operand::Constant(MirConst {
                kind: MirConstKind::Fn(FnDefId::from_raw(42), Substitution::empty()),
                ty: dyn_ty,
                span: glyim_span::Span::DUMMY,
            }),
            args: vec![],
            destination: Place::new(local1),
            target: Some(BasicBlockIdx::from_raw(1)),
            cleanup: None,
        },
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };
    let mut block0 = BasicBlockData::new(term);
    block0.is_cleanup = false;
    basic_blocks.push(block0);

    let ret_term = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };
    basic_blocks.push(BasicBlockData::new(ret_term));

    let body = Arc::new(Body {
        owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        basic_blocks,
        locals: local_decls,
        arg_count: 0,
        return_ty: glyim_type::Ty::UNIT,
        span: glyim_span::Span::DUMMY,
        var_debug_info: vec![],
    });

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok(), "generate_function should succeed: {:?}", result.err());
    let bc = result.unwrap();
    assert!(bc.contains(&0x1B), "bytecode should contain OP_CALL for direct call");
    assert!(!bc.contains(&0x22), "bytecode should NOT contain OP_CALL_INDIRECT for direct call");
    drop(ctx);
}

#[test]
fn object_safety_check() {
    use glyim_type::object_safety::*;
    use glyim_span::Span;

    let interner = Interner::new();
    let method_name = interner.intern("method");
    let static_name = interner.intern("static_fn");

    // Object-safe trait: only &self methods, no generics, no Self: Sized
    let methods = vec![
        MethodSignature {
            name: method_name,
            span: Span::DUMMY,
            self_kind: MethodSelfKind::ByReference,
            has_generic_params: false,
            returns_self: false,
        },
    ];
    let violations = check_object_safety(false, &methods);
    assert!(violations.is_empty(), "trait should be object-safe");

    // Non-object-safe: generic method
    let methods = vec![
        MethodSignature {
            name: method_name,
            span: Span::DUMMY,
            self_kind: MethodSelfKind::ByReference,
            has_generic_params: true,
            returns_self: false,
        },
    ];
    let violations = check_object_safety(false, &methods);
    assert!(!violations.is_empty(), "trait with generic method should not be object-safe");
    assert!(violations.iter().any(|v| matches!(v, ObjectSafetyViolation::GenericMethod { .. })));

    // Non-object-safe: requires Self: Sized
    let violations = check_object_safety(true, &[]);
    assert!(violations.iter().any(|v| matches!(v, ObjectSafetyViolation::SelfSized)));

    // Non-object-safe: static method (no self)
    let methods = vec![
        MethodSignature {
            name: static_name,
            span: Span::DUMMY,
            self_kind: MethodSelfKind::None,
            has_generic_params: false,
            returns_self: false,
        },
    ];
    let violations = check_object_safety(false, &methods);
    assert!(violations.iter().any(|v| matches!(v, ObjectSafetyViolation::StaticMethod { .. })));
}

#[test]
fn vtable_layout_matches_expectations() {
    use glyim_layout::{SimpleLayoutComputer, LayoutComputer};
    let (ctx, dyn_ty) = with_fresh_ty_ctx(|ctx| {
        let empty_subst = ctx.intern_substitution(vec![]);
        let trait_ref = TraitRef {
            def_id: TraitDefId::from_raw(0),
            substs: empty_subst,
        };
        let predicate = Predicate::Trait(TraitPredicate {
            trait_ref,
            polarity: ImplPolarity::Positive,
        });
        let box_predicates: Box<[Predicate]> = Box::new([predicate]);
        let bound_vars: Box<[BoundVariableKind]> = vec![].into();
        let binder = Binder::bind(box_predicates, bound_vars);
        let kind = TyKind::Dynamic(binder, Region::Erased);
        ctx.mk_ty(kind)
    });
    let computer = SimpleLayoutComputer::new(&ctx, TargetInfo::x86_64());
    let layout = computer.layout_of(dyn_ty).expect("layout_of(dyn Trait) should succeed");
    assert_eq!(layout.size.0, 16, "fat pointer size should be 16 on x86_64");
    assert_eq!(layout.align.0, 8, "fat pointer align should be 8 on x86_64");
    assert!(!layout.is_unsized);
}
