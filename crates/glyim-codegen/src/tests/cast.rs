use super::super::*;
use glyim_core::def_id::{CrateId, DefId, LocalDefId};
use glyim_core::primitives::Mutability;
use glyim_span::Span;
use glyim_type::Ty;
use std::sync::Arc;

#[test]
fn emit_cast_int_to_float() {
    let mut body = Body::dummy(DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));
    let dest = LocalIdx::from_raw(1);
    body.locals.push(LocalDecl {
        ty: Ty::ERROR,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    let src_ty = Ty::ERROR; // will be replaced with real type later
    body.basic_blocks[BasicBlockIdx::from_raw(0)]
        .statements
        .push(Statement {
            kind: StatementKind::Assign(
                Place::new(dest),
                Rvalue::Cast(
                    CastKind::IntToFloat,
                    Operand::Constant(MirConst {
                        kind: MirConstKind::Int(10),
                        ty: Ty::ERROR,
                        span: Span::DUMMY,
                    }),
                    src_ty,
                ),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        });
    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&Arc::new(body)).unwrap();
    assert!(result.contains(&OP_CAST));
}

#[test]
fn emit_cast_float_to_int() {
    let mut body = Body::dummy(DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));
    let dest = LocalIdx::from_raw(1);
    body.locals.push(LocalDecl {
        ty: Ty::ERROR,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    body.basic_blocks[BasicBlockIdx::from_raw(0)]
        .statements
        .push(Statement {
            kind: StatementKind::Assign(
                Place::new(dest),
                Rvalue::Cast(
                    CastKind::FloatToInt,
                    Operand::Constant(MirConst {
                        kind: MirConstKind::FloatBits(1.0f64.to_bits()),
                        ty: Ty::ERROR,
                        span: Span::DUMMY,
                    }),
                    Ty::ERROR,
                ),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        });
    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&Arc::new(body)).unwrap();
    assert!(result.contains(&OP_CAST));
}
