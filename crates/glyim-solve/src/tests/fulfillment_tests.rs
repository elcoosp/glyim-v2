use crate::*;
use glyim_core::def_id::{ImplDefId, TraitDefId};
use glyim_core::interner::Interner;
use glyim_core::primitives::{IntTy, UintTy};
use glyim_test::test_ty_ctx;
use glyim_type::*;

#[test]
fn test_fulfillment_ctx_processes_trait_obligations() {
    let mut ctx_mut = test_ty_ctx();
    let ctx = ctx_mut.freeze();
    let mut trait_ctx = solver::TraitContext::new();
    let trait_id = TraitDefId::from_raw(0);
    let interner = Interner::new();
    let trait_name = interner.intern("MyTrait");
    trait_ctx.register_trait(solver::TraitDef {
        def_id: trait_id,
        name: trait_name,
        associated_types: vec![],
        predicates: vec![],
    });
    let impl_id = ImplDefId::from_raw(0);
    let trait_ref = TraitRef {
        def_id: trait_id,
        substs: Substitution::empty(),
    };
    trait_ctx.register_impl(solver::ImplDef {
        def_id: impl_id,
        trait_ref: trait_ref.clone(),
        predicates: vec![],
    });
    let mut solver = solver::SimpleTraitSolver::new(&trait_ctx);
    let mut fulfillment = FulfillmentCtx::new(&ctx, &mut solver);
    let obligation = Obligation {
        predicate: Predicate::Trait(TraitPredicate {
            trait_ref,
            polarity: ImplPolarity::Positive,
        }),
        cause: ObligationCause {
            span: glyim_span::Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    };
    fulfillment.register_obligation(obligation);
    let result = fulfillment.process_obligations(100);
    assert!(result.is_ok(), "Processing should succeed");
    let diags = fulfillment.into_diagnostics();
    assert!(diags.is_empty(), "No diagnostics should be emitted");
}

#[test]
fn test_fulfillment_ctx_coerce_obligation() {
    let mut ctx_mut = test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let usize_ty = ctx_mut.mk_ty(TyKind::Uint(UintTy::Usize));
    let len_const = Const {
        kind: ConstKind::Int(3),
        ty: usize_ty,
    };
    let array_ty = ctx_mut.mk_ty(TyKind::Array(i32_ty, len_const));
    let slice_ty = ctx_mut.mk_ty(TyKind::Slice(i32_ty));
    let ctx = ctx_mut.freeze();
    let trait_ctx = solver::TraitContext::new();
    let mut solver = solver::SimpleTraitSolver::new(&trait_ctx);
    let mut fulfillment = FulfillmentCtx::new(&ctx, &mut solver);
    let obligation = Obligation {
        predicate: Predicate::Coerce(array_ty, slice_ty),
        cause: ObligationCause {
            span: glyim_span::Span::DUMMY,
            code: ObligationCauseCode::WellFormed,
        },
    };
    fulfillment.register_obligation(obligation);
    let result = fulfillment.process_obligations(100);
    assert!(result.is_ok());
    let diags = fulfillment.into_diagnostics();
    assert!(
        diags.is_empty(),
        "Coercion should be proven, no diagnostics"
    );
}
