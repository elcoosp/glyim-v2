use crate::*;

#[test]
fn cast_kind_variants() {
    assert_eq!(CastKind::IntToInt, CastKind::IntToInt);
    assert_eq!(CastKind::FloatToInt, CastKind::FloatToInt);
    assert_eq!(CastKind::IntToFloat, CastKind::IntToFloat);
    assert_eq!(CastKind::PtrToPtr, CastKind::PtrToPtr);
    assert_eq!(CastKind::FnPtrToPtr, CastKind::FnPtrToPtr);
}
