use glyim_core::arena::IndexVec;
use glyim_core::primitives::*;
use glyim_core::{CrateId, DefId, LocalDefId};
use glyim_mir::{
    BasicBlockData, BasicBlockIdx, Body, LocalDecl, LocalIdx, MirConst, MirConstKind, Operand,
    Place, Rvalue, SourceInfo, Statement, StatementKind, Terminator, TerminatorKind,
};
use glyim_span::Span;
use glyim_type::{Ty, TyCtx, TyKind};

pub(crate) fn simple_mir_body(dest_ty: Ty, rvalue: Rvalue) -> Body {
    let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
    let local0 = locals.push(LocalDecl {
        ty: dest_ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    let place = Place::new(local0);

    let stmt = Statement {
        kind: StatementKind::Assign(place, rvalue),
        source_info: SourceInfo::new(Span::DUMMY),
    };

    let mut basic_blocks: IndexVec<BasicBlockIdx, BasicBlockData> = IndexVec::new();
    let bb0 = basic_blocks.push(BasicBlockData::new(Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    }));
    basic_blocks[bb0].statements.push(stmt);

    Body {
        owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        basic_blocks,
        locals,
        arg_count: 0,
        return_ty: Ty::UNIT,
        span: Span::DUMMY,
        var_debug_info: vec![],
    }
}

pub(crate) fn const_operand_i32(val: i64, ctx: &TyCtx) -> Operand {
    let i32_ty = ctx.mk_ty_from_kind(TyKind::Int(IntTy::I32));
    Operand::Constant(MirConst {
        kind: MirConstKind::Int(val as i128),
        ty: i32_ty,
        span: Span::DUMMY,
    })
}

pub(crate) fn const_operand_u32(val: u64, ctx: &TyCtx) -> Operand {
    let u32_ty = ctx.mk_ty_from_kind(TyKind::Uint(UintTy::U32));
    Operand::Constant(MirConst {
        kind: MirConstKind::Uint(val as u128),
        ty: u32_ty,
        span: Span::DUMMY,
    })
}

pub(crate) fn const_operand_bool(val: bool) -> Operand {
    let bool_ty = Ty::BOOL;
    Operand::Constant(MirConst {
        kind: MirConstKind::Bool(val),
        ty: bool_ty,
        span: Span::DUMMY,
    })
}

pub(crate) fn box_operands(left: Operand, right: Operand) -> Box<(Operand, Operand)> {
    Box::new((left, right))
}
