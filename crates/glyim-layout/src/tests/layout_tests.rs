use glyim_test::{assert_layout, test_frozen_ty_ctx, with_fresh_ty_ctx};
use glyim_core::primitives::*;
use crate::*;
use glyim_type::*;

#[test]
fn s04_t01_layout_i8() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Int(IntTy::I8)));
    assert_layout(&ctx, ty, 1, 1);
}

#[test]
fn s04_t02_layout_i16() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Int(IntTy::I16)));
    assert_layout(&ctx, ty, 2, 2);
}

#[test]
fn s04_t03_layout_i32() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Int(IntTy::I32)));
    assert_layout(&ctx, ty, 4, 4);
}

#[test]
fn s04_t04_layout_i64() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Int(IntTy::I64)));
    assert_layout(&ctx, ty, 8, 8);
}

#[test]
fn s04_t05_layout_f32() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Float(FloatTy::F32)));
    assert_layout(&ctx, ty, 4, 4);
}

#[test]
fn s04_t06_layout_f64() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Float(FloatTy::F64)));
    assert_layout(&ctx, ty, 8, 8);
}

#[test]
fn s04_t07_layout_bool() {
    let ctx = test_frozen_ty_ctx();
    assert_layout(&ctx, Ty::BOOL, 1, 1);
}

#[test]
fn s04_t08_layout_unit() {
    let ctx = test_frozen_ty_ctx();
    assert_layout(&ctx, Ty::UNIT, 0, 1);
}

#[test]
fn s04_t09_layout_never() {
    let ctx = test_frozen_ty_ctx();
    assert_layout(&ctx, Ty::NEVER, 0, 1);
}

#[test]
fn s04_t10_layout_ref() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.mk_ref(Region::Erased, c.bool_ty(), Mutability::Not));
    assert_layout(&ctx, ty, 8, 8); // x86_64 pointer
}

#[test]
fn s04_t11_layout_raw_ptr() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::RawPtr(c.bool_ty(), Mutability::Not)));
    assert_layout(&ctx, ty, 8, 8);
}

#[test]
fn s04_t12_layout_slice_is_unsized() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Slice(c.bool_ty())));
    let computer = SimpleLayoutComputer::new(&ctx, TargetInfo::x86_64());
    let result = computer.layout_of(ty);
    assert!(matches!(result, Err(LayoutError::Unsized(_))));
}

#[test]
fn s04_t13_layout_dyn_is_unsized() {
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Dynamic(
        glyim_type::Binder { bound_vars: vec![], value: Box::new([]) },
        Region::Erased,
    )));
    let computer = SimpleLayoutComputer::new(&ctx, TargetInfo::x86_64());
    let result = computer.layout_of(ty);
    assert!(matches!(result, Err(LayoutError::Unsized(_))));
}

#[test]
fn s04_t14_fn_abi_of_basic_signature() {
    let (ctx, fn_sig) = with_fresh_ty_ctx(|c| {
        let inputs = c.intern_substitution(vec![
            GenericArg::Ty(c.mk_ty(TyKind::Int(IntTy::I32))),
            GenericArg::Ty(c.bool_ty()),
        ]);
        FnSig {
            inputs,
            output: c.unit_ty(),
            c_variadic: false,
            unsafety: Safety::Safe,
            abi: Abi::Glyim,
        }
    });
    let computer = SimpleLayoutComputer::new(&ctx, TargetInfo::x86_64());
    let abi = computer.fn_abi_of(&fn_sig).unwrap();
    assert_eq!(abi.args.len(), 2);
    assert_eq!(abi.args[0].layout.size, Size::bytes(4));
    assert_eq!(abi.args[1].layout.size, Size::bytes(1));
    assert_eq!(abi.ret.layout.size, Size::ZERO);
    assert_eq!(abi.conv, CallConvention::Glyim);
}
