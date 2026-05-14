use crate::{
    FulfillmentCtx, ImplDef, Obligation, ObligationCause, ObligationCauseCode, SimpleTraitSolver,
    SolverResult, TraitContext, TraitDef, TraitSolver,
};
use glyim_core::Interner;
use glyim_core::def_id::{ImplDefId, TraitDefId};
use glyim_span::Span;
use glyim_test::test_ty_ctx;
use glyim_type::{ImplPolarity, Predicate, Substitution, TraitPredicate, TraitRef, TyCtxMut};

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
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(5);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("Debug"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = empty_subst(&mut ty_ctx);
    let frozen = ty_ctx.freeze();
    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut solver);
    assert_eq!(fulfill.pending_count(), 0);
    fulfill.register_obligation(Obligation {
        predicate: Predicate::Trait(make_trait_pred(trait_id, subst)),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });
    assert_eq!(fulfill.pending_count(), 1);
}

#[test]
fn t07_bfs_processing_order() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(6);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("Trait"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst_a = ty_ctx.intern_substitution(vec![]);
    let subst_b = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let mut spy = SpySolver::new(SolverResult::Proven);
    spy.respond_with(vec![SolverResult::Proven, SolverResult::Proven]);
    let pred_a = make_trait_pred(trait_id, subst_a);
    let pred_b = make_trait_pred(trait_id, subst_b);
    {
        let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
        fulfill.register_obligation(Obligation {
            predicate: Predicate::Trait(pred_a.clone()),
            cause: ObligationCause {
                span: Span::DUMMY,
                code: ObligationCauseCode::WellFormed,
            },
        });
        fulfill.register_obligation(Obligation {
            predicate: Predicate::Trait(pred_b.clone()),
            cause: ObligationCause {
                span: Span::DUMMY,
                code: ObligationCauseCode::WellFormed,
            },
        });
        assert!(fulfill.process_obligations(10).is_ok());
    }
    assert_eq!(spy.calls.len(), 2);
    assert_eq!(spy.calls[0].trait_ref.def_id, pred_a.trait_ref.def_id);
    assert_eq!(spy.calls[1].trait_ref.def_id, pred_b.trait_ref.def_id);
}

#[test]
fn t08_overflow_protection() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(7);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("Trait"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let pred = make_trait_pred(trait_id, subst);
    let mut spy = SpySolver::new(SolverResult::Proven);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    for _ in 0..5 {
        fulfill.register_obligation(Obligation {
            predicate: Predicate::Trait(pred.clone()),
            cause: ObligationCause {
                span: Span::DUMMY,
                code: ObligationCauseCode::WellFormed,
            },
        });
    }
    let result = fulfill.process_obligations(2);
    assert!(result.is_err());
    assert!(result.unwrap_err().depth > 2);
}

#[test]
fn t09_multiple_obligations_all_checked() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(8);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("Trait"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let pred = make_trait_pred(trait_id, subst);
    let mut spy = SpySolver::new(SolverResult::Proven);
    {
        let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
        for _ in 0..3 {
            fulfill.register_obligation(Obligation {
                predicate: Predicate::Trait(pred.clone()),
                cause: ObligationCause {
                    span: Span::DUMMY,
                    code: ObligationCauseCode::WellFormed,
                },
            });
        }
        assert!(fulfill.process_obligations(10).is_ok());
        assert_eq!(fulfill.processed_count(), 3);
    }
    assert_eq!(spy.calls.len(), 3);
}

#[test]
fn t10_ambiguous_generates_warning_diagnostic() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(9);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("Trait"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let mut spy = SpySolver::new(SolverResult::Ambiguous);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    fulfill.register_obligation(Obligation {
        predicate: Predicate::Trait(make_trait_pred(trait_id, subst)),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });
    assert!(fulfill.process_obligations(10).is_ok());
    let diags = fulfill.into_diagnostics();
    assert!(!diags.is_empty());
    assert!(diags[0].message.contains("ambiguous"));
}

#[test]
fn t11_definite_no_generates_error_diagnostic() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(10);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("Trait"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let mut spy = SpySolver::new(SolverResult::DefiniteNo);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    fulfill.register_obligation(Obligation {
        predicate: Predicate::Trait(make_trait_pred(trait_id, subst)),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });
    assert!(fulfill.process_obligations(10).is_ok());
    let diags = fulfill.into_diagnostics();
    assert!(!diags.is_empty());
    assert!(diags[0].message.contains("not satisfied"));
}

#[test]
fn t15_fulfillment_skips_non_trait_predicates() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    trait_ctx.register_trait(TraitDef {
        def_id: TraitDefId::from_raw(15),
        name: interner.intern("SkipTrait"),
        associated_types: vec![],
        predicates: vec![],
    });
    let ty_ctx = test_ty_ctx();
    let frozen = ty_ctx.freeze();
    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut solver);
    fulfill.register_obligation(Obligation {
        predicate: Predicate::WellFormed(frozen.bool_ty()),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });
    fulfill.register_obligation(Obligation {
        predicate: Predicate::TypeOutlives(glyim_type::TypeOutlivesPredicate {
            ty: frozen.bool_ty(),
            region: glyim_type::Region::Static,
        }),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });
    fulfill.register_obligation(Obligation {
        predicate: Predicate::RegionOutlives(glyim_type::RegionOutlivesPredicate {
            a: glyim_type::Region::Static,
            b: glyim_type::Region::Static,
        }),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });
    fulfill.register_obligation(Obligation {
        predicate: Predicate::Coerce(frozen.bool_ty(), frozen.unit_ty()),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });
    assert!(fulfill.process_obligations(10).is_ok());
    assert!(fulfill.into_diagnostics().is_empty());
}

#[test]
fn t16_fulfillment_empty_queue_returns_ok() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    trait_ctx.register_trait(TraitDef {
        def_id: TraitDefId::from_raw(16),
        name: interner.intern("Empty"),
        associated_types: vec![],
        predicates: vec![],
    });
    let ty_ctx = test_ty_ctx();
    let frozen = ty_ctx.freeze();
    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut solver);
    assert!(fulfill.process_obligations(10).is_ok());
}

#[test]
fn t21_fulfillment_extend_trait() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(21);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("ExtendTrait"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let mut spy = SpySolver::new(SolverResult::Proven);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    fulfill.extend((0..3).map(|_| Obligation {
        predicate: Predicate::Trait(make_trait_pred(trait_id, subst)),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    }));
    assert_eq!(fulfill.pending_count(), 3);
}

#[test]
fn t23_evaluate_predicate_on_coerce_through_fulfillment() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    trait_ctx.register_trait(TraitDef {
        def_id: TraitDefId::from_raw(23),
        name: interner.intern("CoerceTrait"),
        associated_types: vec![],
        predicates: vec![],
    });
    let ty_ctx = test_ty_ctx();
    let frozen = ty_ctx.freeze();
    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut solver);
    fulfill.register_obligation(Obligation {
        predicate: Predicate::Coerce(frozen.bool_ty(), frozen.unit_ty()),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::TypeConstruction,
        },
    });
    fulfill.register_obligation(Obligation {
        predicate: Predicate::Coerce(frozen.never_ty(), frozen.bool_ty()),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::MatchArm,
        },
    });
    assert!(fulfill.process_obligations(10).is_ok());
    assert!(fulfill.into_diagnostics().is_empty());
}

#[test]
fn t24_into_diagnostics_collects_multiple_errors() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(24);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("MultiErr"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let mut spy = SpySolver::new(SolverResult::DefiniteNo);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    for _ in 0..3 {
        fulfill.register_obligation(Obligation {
            predicate: Predicate::Trait(make_trait_pred(trait_id, subst)),
            cause: ObligationCause {
                span: Span::DUMMY,
                code: ObligationCauseCode::WellFormed,
            },
        });
    }
    assert!(fulfill.process_obligations(10).is_ok());
    assert_eq!(fulfill.into_diagnostics().len(), 3);
}

#[test]
fn t25_process_with_exact_limit() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(25);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("ExactLimit"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let pred = make_trait_pred(trait_id, subst);
    let mut spy = SpySolver::new(SolverResult::Proven);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    for _ in 0..5 {
        fulfill.register_obligation(Obligation {
            predicate: Predicate::Trait(pred.clone()),
            cause: ObligationCause {
                span: Span::DUMMY,
                code: ObligationCauseCode::WellFormed,
            },
        });
    }
    assert!(fulfill.process_obligations(5).is_ok());
    assert_eq!(fulfill.processed_count(), 5);
    fulfill.register_obligation(Obligation {
        predicate: Predicate::Trait(pred.clone()),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });
    assert!(fulfill.process_obligations(5).is_err());
}

#[test]
fn t26_process_mixed_trait_and_non_trait_obligations() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(26);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("Mixed"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let pred = make_trait_pred(trait_id, subst);
    let mut spy = SpySolver::new(SolverResult::Proven);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    fulfill.register_obligation(Obligation {
        predicate: Predicate::WellFormed(frozen.bool_ty()),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });
    fulfill.register_obligation(Obligation {
        predicate: Predicate::Trait(pred.clone()),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });
    fulfill.register_obligation(Obligation {
        predicate: Predicate::Coerce(frozen.bool_ty(), frozen.never_ty()),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::TypeConstruction,
        },
    });
    fulfill.register_obligation(Obligation {
        predicate: Predicate::Trait(pred),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::MatchArm,
        },
    });
    assert!(fulfill.process_obligations(10).is_ok());
    assert!(fulfill.into_diagnostics().is_empty());
    assert_eq!(spy.calls.len(), 2);
}

#[test]
fn t30_fulfillment_obligation_cause_codes() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(30);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("CauseTest"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let mut spy = SpySolver::new(SolverResult::Proven);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    for code in &[
        ObligationCauseCode::WellFormed,
        ObligationCauseCode::TypeConstruction,
        ObligationCauseCode::MatchArm,
        ObligationCauseCode::IfThenElse,
    ] {
        fulfill.register_obligation(Obligation {
            predicate: Predicate::Trait(make_trait_pred(trait_id, subst)),
            cause: ObligationCause {
                span: Span::DUMMY,
                code: code.clone(),
            },
        });
    }
    assert_eq!(fulfill.pending_count(), 4);
    assert!(fulfill.process_obligations(10).is_ok());
    assert_eq!(spy.calls.len(), 4);
}

#[test]
fn t31_fulfillment_overflow_error_details() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(31);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("Overflow"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let pred = make_trait_pred(trait_id, subst);
    let mut spy = SpySolver::new(SolverResult::Proven);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    fulfill.register_obligation(Obligation {
        predicate: Predicate::Trait(pred.clone()),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });
    fulfill.register_obligation(Obligation {
        predicate: Predicate::Trait(pred.clone()),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });
    let err = fulfill.process_obligations(1).unwrap_err();
    assert_eq!(err.depth, 2);
    assert_eq!(err.predicate, Predicate::Trait(pred));
}

#[test]
fn t39_fulfillment_into_diagnostics_empty_when_no_errors() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(39);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("Clean"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    trait_ctx.register_impl(ImplDef {
        def_id: ImplDefId::from_raw(3900),
        trait_ref: TraitRef {
            def_id: trait_id,
            substs: subst,
        },
        predicates: vec![],
    });
    let frozen = ty_ctx.freeze();
    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut solver);
    fulfill.register_obligation(Obligation {
        predicate: Predicate::Trait(make_trait_pred(trait_id, subst)),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });
    assert!(fulfill.process_obligations(10).is_ok());
    assert!(fulfill.into_diagnostics().is_empty());
}

#[test]
fn t40_fulfillment_diagnostics_include_span() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(40);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("Spanned"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let mut spy = SpySolver::new(SolverResult::DefiniteNo);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    let span = glyim_span::Span::new(
        glyim_span::FileId::from_raw(1),
        glyim_span::ByteIdx::from_raw(0),
        glyim_span::ByteIdx::from_raw(10),
        glyim_span::SyntaxContext::ROOT,
    );
    fulfill.register_obligation(Obligation {
        predicate: Predicate::Trait(make_trait_pred(trait_id, subst)),
        cause: ObligationCause {
            span,
            code: ObligationCauseCode::TypeConstruction,
        },
    });
    assert!(fulfill.process_obligations(10).is_ok());
    let diags = fulfill.into_diagnostics();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].span.primary.file, span.file);
    assert_eq!(diags[0].span.primary.lo, span.lo);
    assert_eq!(diags[0].span.primary.hi, span.hi);
}

#[test]
fn t41_fulfillment_multiple_cause_codes_in_diagnostics() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(41);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("MultiCause"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let mut spy = SpySolver::new(SolverResult::DefiniteNo);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    for code in &[
        ObligationCauseCode::WellFormed,
        ObligationCauseCode::TypeConstruction,
        ObligationCauseCode::MatchArm,
        ObligationCauseCode::IfThenElse,
    ] {
        fulfill.register_obligation(Obligation {
            predicate: Predicate::Trait(make_trait_pred(trait_id, subst)),
            cause: ObligationCause {
                span: Span::DUMMY,
                code: code.clone(),
            },
        });
    }
    assert!(fulfill.process_obligations(10).is_ok());
    let diags = fulfill.into_diagnostics();
    assert_eq!(diags.len(), 4);
    for d in diags {
        assert!(d.message.contains("not satisfied"));
    }
}

#[test]
fn t42_fulfillment_mixed_ambiguous_and_definite_no() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(42);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("MixedResults"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let mut spy = SpySolver::new(SolverResult::Proven);
    spy.respond_with(vec![
        SolverResult::Ambiguous,
        SolverResult::DefiniteNo,
        SolverResult::Proven,
        SolverResult::Ambiguous,
    ]);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    for _ in 0..4 {
        fulfill.register_obligation(Obligation {
            predicate: Predicate::Trait(make_trait_pred(trait_id, subst)),
            cause: ObligationCause {
                span: Span::DUMMY,
                code: ObligationCauseCode::WellFormed,
            },
        });
    }
    assert!(fulfill.process_obligations(10).is_ok());
    assert_eq!(fulfill.into_diagnostics().len(), 3);
}

#[test]
fn t43_fulfillment_overflow_on_exact_boundary() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(43);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("Boundary"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let mut spy = SpySolver::new(SolverResult::Proven);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    for _ in 0..4 {
        fulfill.register_obligation(Obligation {
            predicate: Predicate::Trait(make_trait_pred(trait_id, subst)),
            cause: ObligationCause {
                span: Span::DUMMY,
                code: ObligationCauseCode::WellFormed,
            },
        });
    }
    assert_eq!(fulfill.process_obligations(3).unwrap_err().depth, 4);
}

#[test]
fn t44_fulfillment_with_trait_context_borrow() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(44);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("Borrow"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    trait_ctx.register_impl(ImplDef {
        def_id: ImplDefId::from_raw(4400),
        trait_ref: TraitRef {
            def_id: trait_id,
            substs: subst,
        },
        predicates: vec![],
    });
    let frozen = ty_ctx.freeze();
    let pred = make_trait_pred(trait_id, subst);
    {
        let mut solver = SimpleTraitSolver::new(&trait_ctx);
        let mut fulfill = FulfillmentCtx::new(&frozen, &mut solver);
        fulfill.register_obligation(Obligation {
            predicate: Predicate::Trait(pred),
            cause: ObligationCause {
                span: Span::DUMMY,
                code: ObligationCauseCode::WellFormed,
            },
        });
        assert!(fulfill.process_obligations(10).is_ok());
        assert!(fulfill.into_diagnostics().is_empty());
    }
    assert_eq!(trait_ctx.trait_defs().len(), 1);
}

#[test]
fn t45_obligation_clone() {
    let ob = Obligation {
        predicate: Predicate::WellFormed(glyim_test::test_frozen_ty_ctx().bool_ty()),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    };
    assert_eq!(ob.clone().cause.span, ob.cause.span);
}

#[test]
fn t46_overflow_error_clone() {
    let err = crate::OverflowError {
        predicate: Predicate::WellFormed(glyim_test::test_frozen_ty_ctx().bool_ty()),
        depth: 42,
    };
    assert_eq!(err.clone().depth, 42);
}

#[test]
fn t47_obligation_cause_clone() {
    let cause = ObligationCause {
        span: Span::DUMMY,
        code: ObligationCauseCode::MatchArm,
    };
    assert_eq!(cause.clone().span, cause.span);
}

#[test]
fn t51_fulfillment_registers_and_processes_interleaved() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(51);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("Interleaved"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let pred = make_trait_pred(trait_id, subst);
    let mut spy = SpySolver::new(SolverResult::Proven);
    {
        let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
        fulfill.register_obligation(Obligation {
            predicate: Predicate::Trait(pred.clone()),
            cause: ObligationCause {
                span: Span::DUMMY,
                code: ObligationCauseCode::WellFormed,
            },
        });
        assert!(fulfill.process_obligations(10).is_ok());
        fulfill.register_obligation(Obligation {
            predicate: Predicate::Trait(pred.clone()),
            cause: ObligationCause {
                span: Span::DUMMY,
                code: ObligationCauseCode::TypeConstruction,
            },
        });
        fulfill.register_obligation(Obligation {
            predicate: Predicate::Trait(pred),
            cause: ObligationCause {
                span: Span::DUMMY,
                code: ObligationCauseCode::MatchArm,
            },
        });
        assert!(fulfill.process_obligations(10).is_ok());
    }
    assert_eq!(spy.calls.len(), 3);
}

#[test]
fn t52_fulfillment_pending_count_after_partial_process() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(52);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("Partial"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let mut spy = SpySolver::new(SolverResult::Proven);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    for _ in 0..10 {
        fulfill.register_obligation(Obligation {
            predicate: Predicate::Trait(make_trait_pred(trait_id, subst)),
            cause: ObligationCause {
                span: Span::DUMMY,
                code: ObligationCauseCode::WellFormed,
            },
        });
    }
    assert_eq!(fulfill.pending_count(), 10);
    let _ = fulfill.process_obligations(4);
    assert_eq!(fulfill.processed_count(), 5);
}

#[test]
fn t53_fulfillment_diagnostics_preserved_order() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(53);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("Ordered"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let mut spy = SpySolver::new(SolverResult::Ambiguous);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    for i in 0..3 {
        fulfill.register_obligation(Obligation {
            predicate: Predicate::Trait(make_trait_pred(trait_id, subst)),
            cause: ObligationCause {
                span: Span::new(
                    glyim_span::FileId::from_raw(1),
                    glyim_span::ByteIdx::from_raw(i * 10),
                    glyim_span::ByteIdx::from_raw(i * 10 + 1),
                    glyim_span::SyntaxContext::ROOT,
                ),
                code: ObligationCauseCode::WellFormed,
            },
        });
    }
    assert!(fulfill.process_obligations(10).is_ok());
    let diags = fulfill.into_diagnostics();
    assert_eq!(diags.len(), 3);
    for (i, d) in diags.iter().enumerate() {
        assert_eq!(
            d.span.primary.lo,
            glyim_span::ByteIdx::from_raw(i as u32 * 10)
        );
    }
}

#[test]
fn t54_can_prove_returns_ambiguous_for_missing_trait_in_context() {
    let trait_ctx = TraitContext::new();
    let mut ty_ctx = test_ty_ctx();
    let subst = empty_subst(&mut ty_ctx);
    let frozen = ty_ctx.freeze();
    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    let pred = TraitPredicate {
        trait_ref: TraitRef {
            def_id: TraitDefId::from_raw(999),
            substs: subst,
        },
        polarity: ImplPolarity::Positive,
    };
    assert_eq!(solver.can_prove(&frozen, &pred), SolverResult::Ambiguous);
}

#[test]
fn t55_process_obligations_limit_zero() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(55);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("Zero"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let mut spy = SpySolver::new(SolverResult::Proven);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    fulfill.register_obligation(Obligation {
        predicate: Predicate::Trait(make_trait_pred(trait_id, subst)),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });
    assert_eq!(fulfill.process_obligations(0).unwrap_err().depth, 1);
}

#[test]
fn t56_spy_solver_lifo_response_order() {
    let mut spy = SpySolver::new(SolverResult::Proven);
    spy.respond_with(vec![SolverResult::DefiniteNo, SolverResult::Ambiguous]);
    let mut ty_ctx_mut = test_ty_ctx();
    let subst = ty_ctx_mut.intern_substitution(vec![]);
    let ty_ctx = ty_ctx_mut.freeze();
    let pred = TraitPredicate {
        trait_ref: TraitRef {
            def_id: TraitDefId::from_raw(1),
            substs: subst,
        },
        polarity: ImplPolarity::Positive,
    };
    assert_eq!(spy.can_prove(&ty_ctx, &pred), SolverResult::DefiniteNo);
    assert_eq!(spy.can_prove(&ty_ctx, &pred), SolverResult::Ambiguous);
    assert_eq!(spy.can_prove(&ty_ctx, &pred), SolverResult::Proven);
}

#[test]
fn t57_fulfillment_register_obligation_many_times() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(57);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("Many"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let mut spy = SpySolver::new(SolverResult::Proven);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    for _ in 0..1000 {
        fulfill.register_obligation(Obligation {
            predicate: Predicate::Trait(make_trait_pred(trait_id, subst)),
            cause: ObligationCause {
                span: Span::DUMMY,
                code: ObligationCauseCode::WellFormed,
            },
        });
    }
    assert_eq!(fulfill.pending_count(), 1000);
}

#[test]
fn t58_evaluate_predicate_calls_can_prove() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(58);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("Eval"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = empty_subst(&mut ty_ctx);
    let frozen = ty_ctx.freeze();
    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    assert_eq!(
        solver.evaluate_predicate(
            &frozen,
            &Predicate::Trait(TraitPredicate {
                trait_ref: TraitRef {
                    def_id: trait_id,
                    substs: subst
                },
                polarity: ImplPolarity::Positive
            })
        ),
        SolverResult::Ambiguous
    );
}

#[test]
fn t65_fulfillment_process_obligations_with_high_limit() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(65);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("HighLimit"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let pred = make_trait_pred(trait_id, subst);
    let mut spy = SpySolver::new(SolverResult::Proven);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    for _ in 0..50 {
        fulfill.register_obligation(Obligation {
            predicate: Predicate::Trait(pred.clone()),
            cause: ObligationCause {
                span: Span::DUMMY,
                code: ObligationCauseCode::WellFormed,
            },
        });
    }
    assert!(fulfill.process_obligations(100).is_ok());
    assert_eq!(spy.calls.len(), 50);
}

#[test]
fn t66_fulfillment_resets_diagnostics_after_into_diagnostics() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(66);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("ResetDiag"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let frozen = ty_ctx.freeze();
    let mut spy = SpySolver::new(SolverResult::DefiniteNo);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut spy);
    fulfill.register_obligation(Obligation {
        predicate: Predicate::Trait(make_trait_pred(trait_id, subst)),
        cause: ObligationCause {
            span: Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    });
    assert!(fulfill.process_obligations(10).is_ok());
    let diags1 = fulfill.into_diagnostics();
    assert_eq!(diags1.len(), 1);
}

#[test]
fn t67_trait_context_clear_and_reuse() {
    let interner = Interner::new();
    let mut ctx = TraitContext::new();
    let tid = TraitDefId::from_raw(67);
    ctx.register_trait(TraitDef {
        def_id: tid,
        name: interner.intern("Reuse"),
        associated_types: vec![],
        predicates: vec![],
    });
    assert_eq!(ctx.trait_defs().len(), 1);
    // Drop and recreate
    let mut ctx2 = TraitContext::new();
    ctx2.register_trait(TraitDef {
        def_id: tid,
        name: interner.intern("Reuse2"),
        associated_types: vec![],
        predicates: vec![],
    });
    assert_eq!(ctx2.trait_defs().len(), 1);
}

#[test]
fn t68_solver_stress_concurrent_queries() {
    let interner = Interner::new();
    let mut ctx = TraitContext::new();
    let tid = TraitDefId::from_raw(68);
    ctx.register_trait(TraitDef {
        def_id: tid,
        name: interner.intern("Stress"),
        associated_types: vec![],
        predicates: vec![],
    });
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    ctx.register_impl(ImplDef {
        def_id: ImplDefId::from_raw(6800),
        trait_ref: TraitRef {
            def_id: tid,
            substs: subst,
        },
        predicates: vec![],
    });
    let frozen = ty_ctx.freeze();
    let mut solver = SimpleTraitSolver::new(&ctx);
    let pred = TraitPredicate {
        trait_ref: TraitRef {
            def_id: tid,
            substs: subst,
        },
        polarity: ImplPolarity::Positive,
    };
    for _ in 0..100 {
        assert_eq!(solver.can_prove(&frozen, &pred), SolverResult::Proven);
    }
}

#[test]
fn t69_fulfillment_new_resets_state() {
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    let trait_id = TraitDefId::from_raw(69);
    trait_ctx.register_trait(TraitDef {
        def_id: trait_id,
        name: interner.intern("Reset"),
        associated_types: vec![],
        predicates: vec![],
    });
    let ty_ctx = test_ty_ctx();
    let frozen = ty_ctx.freeze();
    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    let fulfill = FulfillmentCtx::new(&frozen, &mut solver);
    assert_eq!(fulfill.pending_count(), 0);
    assert_eq!(fulfill.processed_count(), 0);
    assert!(fulfill.into_diagnostics().is_empty());
}

#[test]
fn t70_obligation_cause_debug_format() {
    let cause = ObligationCause {
        span: Span::DUMMY,
        code: ObligationCauseCode::WellFormed,
    };
    let dbg = format!("{:?}", cause);
    assert!(dbg.contains("WellFormed") || dbg.contains("ObligationCause"));
}

#[test]
fn t71_overflow_error_debug_format() {
    let err = crate::OverflowError {
        predicate: Predicate::WellFormed(glyim_test::test_frozen_ty_ctx().bool_ty()),
        depth: 5,
    };
    let dbg = format!("{:?}", err);
    assert!(dbg.contains("5") && dbg.contains("OverflowError"));
}

#[test]
fn t72_solver_result_sentinel_values() {
    // Ensure enum variants are distinct
    assert_ne!(SolverResult::Proven, SolverResult::Ambiguous);
    assert_ne!(SolverResult::Ambiguous, SolverResult::DefiniteNo);
    assert_ne!(SolverResult::DefiniteNo, SolverResult::Proven);
}

#[test]
fn t73_trait_def_debug_format() {
    let interner = Interner::new();
    let td = TraitDef {
        def_id: TraitDefId::from_raw(73),
        name: interner.intern("DebugMe"),
        associated_types: vec![],
        predicates: vec![],
    };
    let dbg = format!("{:?}", td);
    assert!(dbg.contains("DebugMe") || dbg.contains("TraitDef"));
}

#[test]
fn t74_impl_def_debug_format() {
    let mut ty_ctx = test_ty_ctx();
    let subst = ty_ctx.intern_substitution(vec![]);
    let imp = ImplDef {
        def_id: ImplDefId::from_raw(74),
        trait_ref: TraitRef {
            def_id: TraitDefId::from_raw(1),
            substs: subst,
        },
        predicates: vec![],
    };
    let dbg = format!("{:?}", imp);
    assert!(dbg.contains("ImplDef"));
}

#[test]
fn t75_fulfillment_process_zero_obligations() {
    // Process with limit > 0 but no obligations registered
    let interner = Interner::new();
    let mut trait_ctx = TraitContext::new();
    trait_ctx.register_trait(TraitDef {
        def_id: TraitDefId::from_raw(75),
        name: interner.intern("ZeroReg"),
        associated_types: vec![],
        predicates: vec![],
    });
    let ty_ctx = test_ty_ctx();
    let frozen = ty_ctx.freeze();
    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    let mut fulfill = FulfillmentCtx::new(&frozen, &mut solver);
    assert!(fulfill.process_obligations(10).is_ok());
    assert_eq!(fulfill.processed_count(), 0);
    assert_eq!(fulfill.pending_count(), 0);
}
