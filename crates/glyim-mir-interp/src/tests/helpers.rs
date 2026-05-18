use glyim_mir::*;
use glyim_span::Span;
use glyim_type::Ty;

pub fn const_int(val: i128) -> Operand {
    Operand::Constant(MirConst {
        kind: MirConstKind::Int(val),
        ty: Ty::INT,
        span: Span::DUMMY,
    })
}

pub fn const_uint(val: u128) -> Operand {
    Operand::Constant(MirConst {
        kind: MirConstKind::Uint(val),
        ty: Ty::USIZE,
        span: Span::DUMMY,
    })
}

pub fn mir_const_usize(val: u64) -> MirConst {
    MirConst {
        kind: MirConstKind::Uint(val as u128),
        ty: Ty::USIZE,
        span: Span::DUMMY,
    }
}
