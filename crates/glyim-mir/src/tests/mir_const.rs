use crate::*;
use glyim_core::interner::Interner;
use glyim_span::Span;
use glyim_type::Ty;

#[test]
fn mir_const_int() {
    let c = MirConst {
        kind: MirConstKind::Int(-1i128),
        ty: Ty::BOOL,
        span: Span::DUMMY,
    };
    assert!(matches!(c.kind, MirConstKind::Int(-1)));
}

#[test]
fn mir_const_uint() {
    let c = MirConst {
        kind: MirConstKind::Uint(42u128),
        ty: Ty::BOOL,
        span: Span::DUMMY,
    };
    assert!(matches!(c.kind, MirConstKind::Uint(42)));
}

#[test]
fn mir_const_float_bits() {
    let c = MirConst {
        kind: MirConstKind::FloatBits(0x4000000000000000u64),
        ty: Ty::BOOL,
        span: Span::DUMMY,
    };
    assert!(matches!(
        c.kind,
        MirConstKind::FloatBits(0x4000000000000000)
    ));
}

#[test]
fn mir_const_bool() {
    let c = MirConst {
        kind: MirConstKind::Bool(true),
        ty: Ty::BOOL,
        span: Span::DUMMY,
    };
    assert!(matches!(c.kind, MirConstKind::Bool(true)));
}

#[test]
fn mir_const_char() {
    let c = MirConst {
        kind: MirConstKind::Char('a'),
        ty: Ty::BOOL,
        span: Span::DUMMY,
    };
    assert!(matches!(c.kind, MirConstKind::Char('a')));
}

#[test]
fn mir_const_unit() {
    let c = MirConst {
        kind: MirConstKind::Unit,
        ty: Ty::UNIT,
        span: Span::DUMMY,
    };
    assert!(matches!(c.kind, MirConstKind::Unit));
}

#[test]
fn mir_const_error() {
    let c = MirConst {
        kind: MirConstKind::Error,
        ty: Ty::ERROR,
        span: Span::DUMMY,
    };
    assert!(matches!(c.kind, MirConstKind::Error));
}

#[test]
fn mir_const_string() {
    let interner = Interner::new();
    let name = interner.intern("hello");
    let c = MirConst {
        kind: MirConstKind::String(name),
        ty: Ty::BOOL,
        span: Span::DUMMY,
    };
    assert!(matches!(c.kind, MirConstKind::String(_)));
}
