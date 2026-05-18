use crate::*;
use glyim_core::{CrateId, DefId, LocalDefId, Mutability};
use glyim_mir::*;
use glyim_span::Span;
use glyim_type::Ty;

pub fn dummy_def_id() -> DefId {
    DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0))
}

pub fn local_decl(ty: Ty, mutability: Mutability) -> LocalDecl {
    LocalDecl { ty, mutability, source_info: SourceInfo::new(Span::DUMMY) }
}

pub fn empty_body(ret_ty: Ty) -> Body {
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![local_decl(ret_ty, Mutability::Mut)]);
    body
}

pub fn add_local(body: &mut Body, ty: Ty, mutability: Mutability) -> LocalIdx {
    let idx = LocalIdx::from_raw(body.locals.len() as u32);
    body.locals.push(local_decl(ty, mutability));
    idx
}

pub fn add_statement(body: &mut Body, bb: BasicBlockIdx, stmt: StatementKind) {
    while body.basic_blocks.len() <= bb.index() {
        body.basic_blocks.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator { kind: TerminatorKind::Unreachable, source_info: SourceInfo::new(Span::DUMMY) },
            is_cleanup: false,
        });
    }
    body.basic_blocks[bb].statements.push(Statement { kind: stmt, source_info: SourceInfo::new(Span::DUMMY) });
}

pub fn set_terminator(body: &mut Body, bb: BasicBlockIdx, kind: TerminatorKind) {
    while body.basic_blocks.len() <= bb.index() {
        body.basic_blocks.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator { kind: TerminatorKind::Unreachable, source_info: SourceInfo::new(Span::DUMMY) },
            is_cleanup: false,
        });
    }
    body.basic_blocks[bb].terminator.kind = kind;
}
