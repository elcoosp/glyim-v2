use crate::*;
use glyim_core::arena::IndexVec;
use glyim_core::def_id::{CrateId, DefId, LocalDefId};
use glyim_core::primitives::Mutability;
use glyim_span::Span;
use glyim_type::Ty;

fn si() -> SourceInfo {
    SourceInfo::new(Span::DUMMY)
}

#[test]
fn local_decl_ty_matches_body_locals() {
    let mut body = Body::dummy(DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));

    let types = [Ty::BOOL, Ty::UNIT, Ty::NEVER, Ty::ERROR];
    for ty in types {
        body.locals.push(LocalDecl { ty, mutability: Mutability::Not, source_info: si() });
    }

    for (i, expected_ty) in types.iter().enumerate() {
        assert_eq!(body.locals[LocalIdx::from_raw((i + 1) as u32)].ty, *expected_ty);
    }
}

#[test]
fn local_decl_mutability_roundtrip() {
    let decl_not = LocalDecl { ty: Ty::BOOL, mutability: Mutability::Not, source_info: si() };
    assert!(!decl_not.mutability.is_mut());

    let decl_mut = LocalDecl { ty: Ty::BOOL, mutability: Mutability::Mut, source_info: si() };
    assert!(decl_mut.mutability.is_mut());
}

#[test]
fn local_decl_source_info_preserved() {
    let span = Span::new(
        glyim_span::FileId::from_raw(42),
        glyim_span::ByteIdx::from_raw(100),
        glyim_span::ByteIdx::from_raw(200),
        glyim_span::SyntaxContext::ROOT,
    );
    let decl = LocalDecl { ty: Ty::BOOL, mutability: Mutability::Not, source_info: SourceInfo::new(span) };
    assert_eq!(decl.source_info.span.lo.to_usize(), 100);
    assert_eq!(decl.source_info.span.hi.to_usize(), 200);
}

#[test]
fn index_vec_local_decl_push_ordering() {
    let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
    for i in 0..5u32 {
        locals.push(LocalDecl {
            ty: if i % 2 == 0 { Ty::BOOL } else { Ty::UNIT },
            mutability: Mutability::Not,
            source_info: si(),
        });
    }

    assert_eq!(locals.len(), 5);
    for i in 0..5u32 {
        let idx = LocalIdx::from_raw(i);
        let expected_ty = if i % 2 == 0 { Ty::BOOL } else { Ty::UNIT };
        assert_eq!(locals[idx].ty, expected_ty, "Mismatch at local {}", i);
    }
}
