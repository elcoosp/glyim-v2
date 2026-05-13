use glyim_core::def_id::{ImplDefId, TraitDefId};
use glyim_core::Interner;
use glyim_solve::{
    FulfillmentCtx, ImplDef, Obligation, ObligationCause, ObligationCauseCode, SimpleTraitSolver,
    SolverResult, TraitContext, TraitDef, TraitPredicate, TraitRef,
};
use glyim_span::Span;
use glyim_test::test_ty_ctx;
use glyim_type::{ImplPolarity, Substitution, TyCtxMut};

use super::spy_solver::SpySolver;

fn empty_subst(ctx: &mut TyCtxMut) -> Substitution {
    ctx.intern_substitution(vec![])
}

fn make_trait_pred(trait_id: TraitDefId, substs: Substitution) -> TraitPredicate {
    TraitPredicate {
        trait_ref: TraitRef {
            def_id: trait_id,
            substs,
        },
        polarity: ImplPolarity::Positive,
    }
}

#[test]
fn t06_fulfillment_registers_obligations() {
    let mut interner = Interner::new();
    let name = interner.intern("Debug");
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(5);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name,
        associated_types: vec![],
        predicates: vec![],
    });
    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    let mut ty_ctx = test_ty_ctx();
    let subst = empty_subst(&mut ty_ctx);
    let frozen = ty_ctx.freeze();

    let mut fulfill = FulfillmentCtx::new(&frozen, &mut solver);
    assert_eq!(fulfill.pending_count(), 0);
    fulfill.register_obligation(Obligation {
        predicate: glyim_type::Predicate::Trait(make_trait_pred(trait_id, subst)),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });
    assert_eq!(fulfill.pending_count(), 1);
}

#[test]
fn t07_bfs_processing_order() {
    let mut interner = Interner::new();
    let name = interner.intern("Trait");
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(6);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name,
        associated_types: vec![],
        predicates: vec![],
    });

    let mut ty_ctx = test_ty_ctx();
    let subst_a = ty_ctx.intern_substitution(vec![]);
    let subst_b = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();

    let mut spy = SpySolver::new(SolverResult::Proven);
    spy.respond_with(vec![SolverResult::Proven, SolverResult::Proven]);

    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    let pred_a = make_trait_pred(trait_id, subst_a);
    let pred_b = make_trait_pred(trait_id, subst_b);

    fulfill.register_obligation(Obligation {
        predicate: glyim_type::Predicate::Trait(pred_a.clone()),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });
    fulfill.register_obligation(Obligation {
        predicate: glyim_type::Predicate::Trait(pred_b.clone()),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });

    let result = fulfill.process_obligations(10);
    assert!(result.is_ok());

    // BFS order: first registered should be called first
    assert_eq!(spy.calls.len(), 2);
    assert_eq!(spy.calls[0].trait_ref.def_id, pred_a.trait_ref.def_id);
    assert_eq!(spy.calls[1].trait_ref.def_id, pred_b.trait_ref.def_id);
}

#[test]
fn t08_overflow_protection() {
    let mut interner = Interner::new();
    let name = interner.intern("Trait");
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(7);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name,
        associated_types: vec![],
        predicates: vec![],
    });

    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let pred = make_trait_pred(trait_id, subst);

    let mut spy = SpySolver::new(SolverResult::Proven);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);

    // Register several obligations
    for _ in 0..5 {
        fulfill.register_obligation(Obligation {
            predicate: glyim_type::Predicate::Trait(pred.clone()),
            cause: ObligationCause {
                span: Span::DUMMY,
                code: ObligationCauseCode::WellFormed,
            },
        });
    }

    let result = fulfill.process_obligations(2); // limit 2
    assert!(result.is_err());
    let overflow = result.unwrap_err();
    assert!(overflow.depth > 2);
}

#[test]
fn t09_multiple_obligations_all_checked() {
    let mut interner = Interner::new();
    let name = interner.intern("Trait");
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(8);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name,
        associated_types: vec![],
        predicates: vec![],
    });

    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let pred = make_trait_pred(trait_id, subst);

    let mut spy = SpySolver::new(SolverResult::Proven);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);

    for _ in 0..3 {
        fulfill.register_obligation(Obligation {
            predicate: glyim_type::Predicate::Trait(pred.clone()),
            cause: ObligationCause {
                span: Span::DUMMY,
                code: ObligationCauseCode::WellFormed,
            },
        });
    }

    let result = fulfill.process_obligations(10);
    assert!(result.is_ok());
    assert_eq!(spy.calls.len(), 3);
    assert_eq!(fulfill.processed_count(), 3);
}

#[test]
fn t10_ambiguous_generates_warning_diagnostic() {
    let mut interner = Interner::new();
    let name = interner.intern("Trait");
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(9);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name,
        associated_types: vec![],
        predicates: vec![],
    });

    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let pred = make_trait_pred(trait_id, subst);

    let mut spy = SpySolver::new(SolverResult::Ambiguous);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    fulfill.register_obligation(Obligation {
        predicate: glyim_type::Predicate::Trait(pred),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });

    let result = fulfill.process_obligations(10);
    assert!(result.is_ok());
    let diags = fulfill.into_diagnostics();
    assert!(!diags.is_empty());
    let msg = &diags[0].message;
    assert!(
        msg.contains("ambiguous"),
        "expected ambiguous warning, got: {}",
        msg
    );
}

#[test]
fn t11_definite_no_generates_error_diagnostic() {
    let mut interner = Interner::new();
    let name = interner.intern("Trait");
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(10);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name,
        associated_types: vec![],
        predicates: vec![],
    });

    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let pred = make_trait_pred(trait_id, subst);

    let mut spy = SpySolver::new(SolverResult::DefiniteNo);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    fulfill.register_obligation(Obligation {
        predicate: glyim_type::Predicate::Trait(pred),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });

    let result = fulfill.process_obligations(10);
    assert!(result.is_ok());
    let diags = fulfill.into_diagnostics();
    assert!(!diags.is_empty());
    let msg = &diags[0].message;
    assert!(
        msg.contains("not satisfied"),
        "expected error, got: {}",
        msg
    );
}
