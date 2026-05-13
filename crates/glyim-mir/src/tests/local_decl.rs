use crate::*;
use glyim_core::primitives::Mutability;
use glyim_span::Span;
use glyim_type::Ty;

#[test]
fn local_decl_immutable() {
    let decl = LocalDecl {
        ty: Ty::BOOL,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    };
    assert_eq!(decl.ty, Ty::BOOL);
    assert_eq!(decl.mutability, Mutability::Not);
}

#[test]
fn local_decl_mutable() {
    let decl = LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    };
    assert_eq!(decl.ty, Ty::UNIT);
    assert_eq!(decl.mutability, Mutability::Mut);
}

#[test]
fn local_decl_error_ty() {
    let decl = LocalDecl {
        ty: Ty::ERROR,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    };
    assert_eq!(decl.ty, Ty::ERROR);
}

#[test]
fn local_decl_never_ty() {
    let decl = LocalDecl {
        ty: Ty::NEVER,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    };
    assert_eq!(decl.ty, Ty::NEVER);
}
