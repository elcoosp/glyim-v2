use super::super::*;
use glyim_core::def_id::{CrateId, DefId, LocalDefId};
use glyim_core::primitives::Mutability;
use glyim_span::Span;
use glyim_type::Ty;
use std::sync::Arc;

#[test]
fn emit_discriminant() {
    let mut body = Body::dummy(DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));
    let src = LocalIdx::from_raw(1);
    let dest = LocalIdx::from_raw(2);
    body.locals.push(LocalDecl {
        ty: Ty::ERROR,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    body.locals.push(LocalDecl {
        ty: Ty::ERROR,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    body.basic_blocks[BasicBlockIdx::from_raw(0)]
        .statements
        .push(Statement {
            kind: StatementKind::Assign(Place::new(dest), Rvalue::Discriminant(Place::new(src))),
            source_info: SourceInfo::new(Span::DUMMY),
        });
    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&Arc::new(body)).unwrap();
    assert!(result.contains(&OP_DISCRIMINANT));
}

#[test]
fn emit_len() {
    let mut body = Body::dummy(DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));
    let src = LocalIdx::from_raw(1);
    let dest = LocalIdx::from_raw(2);
    body.locals.push(LocalDecl {
        ty: Ty::ERROR,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    body.locals.push(LocalDecl {
        ty: Ty::ERROR,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    body.basic_blocks[BasicBlockIdx::from_raw(0)]
        .statements
        .push(Statement {
            kind: StatementKind::Assign(Place::new(dest), Rvalue::Len(Place::new(src))),
            source_info: SourceInfo::new(Span::DUMMY),
        });
    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&Arc::new(body)).unwrap();
    assert!(result.contains(&OP_LEN));
}
