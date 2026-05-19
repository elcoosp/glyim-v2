use crate::interner::Interner;

#[test]
fn intern_resolve_lookup() {
    let interner = Interner::new();
    let name = interner.intern("hello");
    assert_eq!(interner.resolve(name), "hello");
    let maybe = interner.lookup("hello");
    assert!(maybe.is_some());
    assert_eq!(maybe.unwrap(), name);
    let none = interner.lookup("unknown");
    assert!(none.is_none());
}

#[test]
fn interner_clone_default() {
    let interner = Interner::new();
    let name = interner.intern("clone_test");
    let clone = interner.clone();
    assert_eq!(clone.resolve(name), "clone_test");
    let default = Interner::default();
    let name2 = default.intern("default");
    assert_eq!(default.resolve(name2), "default");
}
