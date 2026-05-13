use glyim_core::def_id::{ImplDefId, TraitDefId};
use glyim_core::interner::Name;
use glyim_core::Interner;
use glyim_solve::{
    ImplDef, SimpleTraitSolver, SolverResult, TraitContext, TraitDef, TraitPredicate, TraitRef,
};
use glyim_test::test_ty_ctx;
use glyim_type::{ImplPolarity, Substitution, TyCtxMut};

fn empty_subst(ctx: &mut TyCtxMut) -> Substitution {
    ctx.intern_substitution(vec![])
}

#[test]
fn t01_register_trait() {
    let mut interner = Interner::new();
    let name = interner.intern("MyTrait");
    let mut ctx = TraitContext::new();
    let trait_def = TraitDef {
        def_id: TraitDefId::from_raw(1),
        name,
        associated_types: vec![],
        predicates: vec![],
    };
    ctx.register_trait(trait_def.clone());
    assert_eq!(ctx.trait_defs().len(), 1);
    assert_eq!(ctx.trait_defs()[0].name, name);
}

#[test]
fn t02_register_impl() {
    let mut interner = Interner::new();
    let trait_name = interner.intern("Display");
    let mut ctx = TraitContext::new();
    let trait_def = TraitDef {
        def_id: TraitDefId::from_raw(10),
        name: trait_name,
        associated_types: vec![],
        predicates: vec![],
    };
    ctx.register_trait(trait_def);

    let mut ctx_mut = test_ty_ctx();
    let subst = empty_subst(&mut ctx_mut);
    let impl_def = ImplDef {
        def_id: ImplDefId::from_raw(100),
        trait_ref: TraitRef {
            def_id: TraitDefId::from_raw(10),
            substs: subst,
        },
        predicates: vec![],
    };
    ctx.register_impl(impl_def);
    assert_eq!(ctx.impl_defs().len(), 1);
    assert_eq!(ctx.impl_defs()[0].trait_ref.def_id, TraitDefId::from_raw(10));
}

#[test]
fn t03_prove_with_matching_impl() {
    let mut interner = Interner::new();
    let name = interner.intern("Clone");
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(2);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name,
        associated_types: vec![],
        predicates: vec![],
    });

    let mut ty_ctx = test_ty_ctx();
    let subst = empty_subst(&mut ty_ctx);
    trait_ctx.register_impl(ImplDef {
        def_id: ImplDefId::from_raw(200),
        trait_ref: TraitRef {
            def_id: trait_id,
            substs: subst,
        },
        predicates: vec![],
    });

    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    let trait_pred = TraitPredicate {
        trait_ref: TraitRef {
            def_id: trait_id,
            substs: ty_ctx.intern_substitution(vec![]),
        },
        polarity: ImplPolarity::Positive,
    };
    let result = solver.can_prove(&ty_ctx.freeze(), &trait_pred);
    assert_eq!(result, SolverResult::Proven);
}

#[test]
fn t04_prove_no_impl_gives_ambiguous() {
    let mut interner = Interner::new();
    let name = interner.intern("Default");
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(3);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name,
        associated_types: vec![],
        predicates: vec![],
    });

    let mut ty_ctx = test_ty_ctx();
    let subst = empty_subst(&mut ty_ctx);
    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    let trait_pred = TraitPredicate {
        trait_ref: TraitRef {
            def_id: trait_id,
            substs: subst,
        },
        polarity: ImplPolarity::Positive,
    };
    let result = solver.can_prove(&ty_ctx.freeze(), &trait_pred);
    assert_eq!(result, SolverResult::Ambiguous);
}

#[test]
fn t05_impls_of_trait_subset() {
    let mut interner = Interner::new();
    let name_a = interner.intern("TraitA");
    let name_b = interner.intern("TraitB");
    let mut ctx = TraitContext::new();

    let id_a = TraitDefId::from_raw(10);
    let id_b = TraitDefId::from_raw(20);
    ctx.register_trait(TraitDef {
        def_id: id_a,
        name: name_a,
        associated_types: vec![],
        predicates: vec![],
    });
    ctx.register_trait(TraitDef {
        def_id: id_b,
        name: name_b,
        associated_types: vec![],
        predicates: vec![],
    });

    let mut ty_ctx = test_ty_ctx();
    let subst_a = ty_ctx.intern_substitution(vec![]);
    ctx.register_impl(ImplDef {
        def_id: ImplDefId::from_raw(1),
        trait_ref: TraitRef {
            def_id: id_a,
            substs: subst_a,
        },
        predicates: vec![],
    });

    let subst_b = ty_ctx.intern_substitution(vec![]);
    ctx.register_impl(ImplDef {
        def_id: ImplDefId::from_raw(2),
        trait_ref: TraitRef {
            def_id: id_b,
            substs: subst_b,
        },
        predicates: vec![],
    });

    ctx.register_impl(ImplDef {
        def_id: ImplDefId::from_raw(3),
        trait_ref: TraitRef {
            def_id: id_a,
            substs: ty_ctx.intern_substitution(vec![]),
        },
        predicates: vec![],
    });

    let a_impls: Vec<_> = ctx
        .impls_of_trait(id_a)
        .map(|i| i.def_id)
        .collect();
    assert_eq!(a_impls.len(), 2);
    assert!(a_impls.contains(&ImplDefId::from_raw(1)));
    assert!(a_impls.contains(&ImplDefId::from_raw(3)));

    let b_impls: Vec<_> = ctx
        .impls_of_trait(id_b)
        .map(|i| i.def_id)
        .collect();
    assert_eq!(b_impls.len(), 1);
    assert_eq!(b_impls[0], ImplDefId::from_raw(2));
}
