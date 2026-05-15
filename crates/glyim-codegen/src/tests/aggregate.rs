use super::super::*;
use glyim_core::def_id::{CrateId, DefId, LocalDefId};
use glyim_core::primitives::Mutability;
use glyim_span::Span;
use glyim_type::Ty;
use std::sync::Arc;

#[test]
fn emit_aggregate_tuple() {
    let mut body = Body::dummy(DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));
    let dest = LocalIdx::from_raw(1);
    body.locals.push(LocalDecl {
        ty: Ty::ERROR,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    let operands = vec![
        Operand::Constant(MirConst {
            kind: MirConstKind::Int(1),
            ty: Ty::ERROR,
            span: Span::DUMMY,
        }),
        Operand::Constant(MirConst {
            kind: MirConstKind::Bool(false),
            ty: Ty::BOOL,
            span: Span::DUMMY,
        }),
    ];
    body.basic_blocks[BasicBlockIdx::from_raw(0)]
        .statements
        .push(Statement {
            kind: StatementKind::Assign(
                Place::new(dest),
                Rvalue::Aggregate(AggregateKind::Tuple, operands),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        });
    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&Arc::new(body)).unwrap();
    assert!(result.contains(&OP_AGGREGATE));
    // Verify count field follows OP_AGGREGATE (2)
    let pos = result.iter().position(|&b| b == OP_AGGREGATE).unwrap();
    let count = u32::from_le_bytes([
        result[pos + 1],
        result[pos + 2],
        result[pos + 3],
        result[pos + 4],
    ]);
    assert_eq!(count, 2);
}
