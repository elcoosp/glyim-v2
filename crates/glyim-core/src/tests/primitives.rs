use crate::{
    Abi, BinOp, FloatTy, IntTy, Mutability, Safety, StructKind, TargetInfo, UintTy, UnOp,
    Visibility,
};

fn dummy_target() -> TargetInfo {
    TargetInfo::x86_64()
}

#[test]
fn int_ty_bit_width() {
    let target = dummy_target();
    assert_eq!(IntTy::I8.bit_width(&target), 8);
    assert_eq!(IntTy::I16.bit_width(&target), 16);
    assert_eq!(IntTy::I32.bit_width(&target), 32);
    assert_eq!(IntTy::I64.bit_width(&target), 64);
    assert_eq!(IntTy::Isize.bit_width(&target), 64);
    assert_eq!(IntTy::I8.name(), "i8");
    assert_eq!(IntTy::Isize.name(), "isize");
}

#[test]
fn uint_ty_bit_width() {
    let target = dummy_target();
    assert_eq!(UintTy::U8.bit_width(&target), 8);
    assert_eq!(UintTy::U16.bit_width(&target), 16);
    assert_eq!(UintTy::U32.bit_width(&target), 32);
    assert_eq!(UintTy::U64.bit_width(&target), 64);
    assert_eq!(UintTy::Usize.bit_width(&target), 64);
    assert_eq!(UintTy::U8.name(), "u8");
    assert_eq!(UintTy::Usize.name(), "usize");
}

#[test]
fn float_ty() {
    assert_eq!(FloatTy::F32.bit_width(), 32);
    assert_eq!(FloatTy::F64.bit_width(), 64);
    assert_eq!(FloatTy::F32.name(), "f32");
    assert_eq!(FloatTy::F64.name(), "f64");
}

#[test]
fn mutability() {
    assert!(!Mutability::Not.is_mut());
    assert!(Mutability::Mut.is_mut());
    assert_eq!(Mutability::Not.prefix_str(), "");
    assert_eq!(Mutability::Mut.prefix_str(), "mut ");
}

#[test]
fn safety_abi() {
    assert!(matches!(Safety::Safe, Safety::Safe));
    assert!(matches!(Safety::Unsafe, Safety::Unsafe));
    assert_eq!(Abi::C.name(), "C");
    assert_eq!(Abi::Glyim.name(), "glyim");
    assert_eq!(Abi::System.name(), "system");
}

#[test]
fn binop_comparison() {
    assert!(BinOp::Eq.is_comparison());
    assert!(BinOp::Ne.is_comparison());
    assert!(BinOp::Lt.is_comparison());
    assert!(!BinOp::Add.is_comparison());
    assert!(!BinOp::Mul.is_comparison());
}

#[test]
fn unop() {
    assert!(matches!(UnOp::Not, UnOp::Not));
    assert!(matches!(UnOp::Neg, UnOp::Neg));
    assert!(matches!(UnOp::Deref, UnOp::Deref));
}

#[test]
fn visibility_struct_kind() {
    assert!(matches!(Visibility::Public, Visibility::Public));
    assert!(matches!(Visibility::Module(5), Visibility::Module(5)));
    assert!(matches!(Visibility::Inherited, Visibility::Inherited));
    assert!(matches!(StructKind::Unit, StructKind::Unit));
    assert!(matches!(StructKind::Tuple, StructKind::Tuple));
    assert!(matches!(StructKind::Record, StructKind::Record));
}
