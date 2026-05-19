use crate::interner::Interner;
use crate::path::{Path, PathKind, PathSegment};

#[test]
fn from_single_as_name() {
    let interner = Interner::new();
    let name = interner.intern("foo");
    let path = Path::from_single(name);
    assert_eq!(path.segments.len(), 1);
    assert_eq!(path.segments[0].name, name);
    assert_eq!(path.kind, PathKind::Plain);
    assert_eq!(path.as_name(), Some(name));
}

#[test]
fn as_name_none() {
    let interner = Interner::new();
    let name = interner.intern("foo");
    let mut path = Path::from_single(name);
    path.kind = PathKind::SelfPath;
    assert_eq!(path.as_name(), None);
    path.kind = PathKind::Plain;
    path.segments.push(PathSegment { name });
    assert_eq!(path.as_name(), None);
}
