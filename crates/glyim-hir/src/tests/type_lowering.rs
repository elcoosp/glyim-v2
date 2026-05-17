use glyim_core::interner::Interner;
use glyim_core::primitives::*;
use glyim_span::FileId;
use glyim_frontend::parse_to_syntax;
use glyim_syntax::SyntaxNode;
use crate::lower::lower_type_ref;
use crate::TypeRef;

fn parse_type(src: &str) -> SyntaxNode {
    let parse = parse_to_syntax(src, FileId::from_raw(1)).unwrap();
    parse.root
        .children()
        .find(|n| n.kind().is_type())
        .expect("type node not found")
        .clone()
}

#[test]
fn test_lower_slice_type() {
    let node = parse_type("&[i32]");
    let mut interner = Interner::default();
    let ty = lower_type_ref(&node, &mut interner).unwrap();

    match ty {
        TypeRef::Ref { inner, mutability } => {
            assert_eq!(mutability, Mutability::Not);
            match *inner {
                TypeRef::Slice(elem) => {
                    match *elem {
                        TypeRef::Path(path) => {
                            assert_eq!(path.as_name(), Some(interner.intern("i32")));
                        }
                        _ => panic!("expected Path for i32"),
                    }
                }
                _ => panic!("expected Slice"),
            }
        }
        _ => panic!("expected Ref"),
    }
}

#[test]
fn test_lower_array_type() {
    let node = parse_type("[i32; 5]");
    let mut interner = Interner::default();
    let ty = lower_type_ref(&node, &mut interner).unwrap();

    match ty {
        TypeRef::Array { inner, len: _ } => {
            match *inner {
                TypeRef::Path(path) => {
                    assert_eq!(path.as_name(), Some(interner.intern("i32")));
                }
                _ => panic!("expected Path for i32"),
            }
        }
        _ => panic!("expected Array"),
    }
}

#[test]
fn test_lower_dyn_type() {
    let node = parse_type("dyn Clone");
    let mut interner = Interner::default();
    let ty = lower_type_ref(&node, &mut interner).unwrap();

    match ty {
        TypeRef::Path(path) => {
            assert_eq!(path.as_name(), Some(interner.intern("Clone")));
        }
        _ => panic!("expected Path for dyn trait"),
    }
}

#[test]
fn test_lower_fn_ptr_type() {
    let node = parse_type("fn(i32, bool) -> u64");
    let mut interner = Interner::default();
    let ty = lower_type_ref(&node, &mut interner).unwrap();

    match ty {
        TypeRef::Fn { params, ret } => {
            assert_eq!(params.len(), 2);
            match &params[0] {
                TypeRef::Path(p) => assert_eq!(p.as_name(), Some(interner.intern("i32"))),
                _ => panic!("expected i32 param"),
            }
            match &params[1] {
                TypeRef::Path(p) => assert_eq!(p.as_name(), Some(interner.intern("bool"))),
                _ => panic!("expected bool param"),
            }
            match ret.as_deref() {
                Some(TypeRef::Path(p)) => assert_eq!(p.as_name(), Some(interner.intern("u64"))),
                _ => panic!("expected u64 return"),
            }
        }
        _ => panic!("expected Fn type"),
    }
}
