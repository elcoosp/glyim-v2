use super::super::*;
use glyim_core::def_id::{CrateId, DefId, LocalDefId};
use glyim_core::primitives::{Mutability, UnOp};
use glyim_span::Span;
use glyim_type::Ty;
use std::sync::Arc;

fn make_unary_body(op: UnOp) -> Body {
    let mut body = Body::dummy(DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));
    let dest_local = LocalIdx::from_raw(1);
    body.locals.push(LocalDecl {
        ty: Ty::ERROR,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    body.basic_blocks[BasicBlockIdx::from_raw(0)]
        .statements
        .push(Statement {
            kind: StatementKind::Assign(
                Place::new(dest_local),
                Rvalue::UnaryOp(
                    op,
                    Operand::Constant(MirConst {
                        kind: MirConstKind::Bool(true),
                        ty: Ty::BOOL,
                        span: Span::DUMMY,
                    }),
                ),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        });
    body
}

#[test]
fn emit_not() {
    let body = make_unary_body(UnOp::Not);
    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&Arc::new(body)).unwrap();
    assert!(result.contains(&OP_NOT));
}

#[test]
fn emit_neg() {
    let body = make_unary_body(UnOp::Neg);
    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&Arc::new(body)).unwrap();
    assert!(result.contains(&OP_NEG));
}
