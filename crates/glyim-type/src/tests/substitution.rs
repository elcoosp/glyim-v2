use glyim_core::primitives::{IntTy, UintTy};

use super::helpers::with_fresh_ty_ctx;
use crate::*;

// S02-T09: Substitution interning deduplicates

#[test]
fn substitution_deduplicates_identical_args() {
    let (_ctx, (sub1, sub2)) = with_fresh_ty_ctx(|c| {
        let args = vec![
            GenericArg::Ty(c.bool_ty()),
            GenericArg::Ty(c.mk_ty(TyKind::Int(IntTy::I32))),
        ];
        let s1 = c.intern_substitution(args.clone());
        let s2 = c.intern_substitution(args);
        (s1, s2)
    });
    assert_eq!(sub1.index(), sub2.index());
    assert_eq!(sub1.len(), sub2.len());
}

#[test]
fn substitution_different_args_different_index() {
    let (_ctx, (sub1, sub2)) = with_fresh_ty_ctx(|c| {
        let args1 = vec![GenericArg::Ty(c.bool_ty())];
        let args2 = vec![GenericArg::Ty(c.mk_ty(TyKind::Int(IntTy::I32)))];
        let s1 = c.intern_substitution(args1);
        let s2 = c.intern_substitution(args2);
        (s1, s2)
    });
    assert_ne!(sub1.index(), sub2.index());
}

// S02-T10: Substitution with 10 args

#[test]
fn substitution_with_10_args() {
    let (ctx, sub) = with_fresh_ty_ctx(|c| {
        let args: Vec<GenericArg> = (0..10)
            .map(|_| GenericArg::Ty(c.mk_ty(TyKind::Int(IntTy::I32))))
            .collect();
        c.intern_substitution(args)
    });
    assert_eq!(sub.len(), 10);
    let args = ctx.substitution_args(sub);
    assert_eq!(args.len(), 10);
    for arg in args {
        if let GenericArg::Ty(t) = arg {
            assert!(matches!(ctx.ty_kind(*t), TyKind::Int(IntTy::I32)));
        } else {
            panic!("expected Ty arg");
        }
    }
}

#[test]
fn empty_substitution() {
    let (ctx, sub) = with_fresh_ty_ctx(|c| c.intern_substitution(vec![]));
    assert!(sub.is_empty());
    assert_eq!(sub.len(), 0);
    let args = ctx.substitution_args(sub);
    assert!(args.is_empty());
}

#[test]
fn substitution_with_lifetime_arg() {
    let (ctx, sub) = with_fresh_ty_ctx(|c| {
        let args = vec![
            GenericArg::Lifetime(Region::Erased),
            GenericArg::Ty(c.bool_ty()),
        ];
        c.intern_substitution(args)
    });
    let args = ctx.substitution_args(sub);
    assert_eq!(args.len(), 2);
    assert!(matches!(&args[0], GenericArg::Lifetime(Region::Erased)));
    assert!(matches!(&args[1], GenericArg::Ty(t) if *t == Ty::BOOL));
}

#[test]
fn substitution_with_const_arg() {
    let (ctx, sub) = with_fresh_ty_ctx(|c| {
        let const_val = Const {
            kind: ConstKind::Uint(42),
            ty: c.mk_ty(TyKind::Uint(UintTy::U32)),
        };
        let args = vec![GenericArg::Ty(c.bool_ty()), GenericArg::Const(const_val)];
        c.intern_substitution(args)
    });
    let args = ctx.substitution_args(sub);
    assert_eq!(args.len(), 2);
    assert!(matches!(&args[1], GenericArg::Const(_)));
}
