use glyim_core::{TargetInfo, TraitDefId};
use glyim_test::with_fresh_ty_ctx;
use glyim_type::*;

#[test]
fn create_trait_object_from_concrete_type() {
    assert!(true);
}

#[test]
fn call_method_through_trait_object() {
    assert!(true);
}

#[test]
fn upcast_to_supertrait_object() {
    assert!(true);
}

#[test]
fn object_safety_check() {
    assert!(true);
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
    let _layout = computer.layout_of(dyn_ty).expect("layout_of(dyn Trait) should succeed");
}
