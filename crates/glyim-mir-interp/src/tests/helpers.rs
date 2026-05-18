use glyim_mir::*;
use glyim_span::Span;
use glyim_type::{Ty, TyKind, IntTy, UintTy};

/// Create an Operand::Constant with an integer value (i128).
/// Uses Ty::INT as a dummy type (interpreter will still work).
pub fn const_int(val: i128) -> Operand {
    Operand::Constant(MirConst {
        kind: MirConstKind::Int(val),
        ty: Ty::INT,
        span: Span::DUMMY,
    })
}

/// Create a MirConst with a usize value (as Uint).
pub fn mir_const_usize(val: u64) -> MirConst {
    MirConst {
        kind: MirConstKind::Uint(val),
        ty: Ty::USIZE,
        span: Span::DUMMY,
    }
}

/// Create an Operand::Constant from a MirConst.
pub fn const_mir(c: MirConst) -> Operand {
    Operand::Constant(c)
}

/// Create a unit constant (for placeholder).
pub fn const_unit() -> Operand {
    Operand::Constant(MirConst {
        kind: MirConstKind::Unit,
        ty: Ty::UNIT,
        span: Span::DUMMY,
    })
}
