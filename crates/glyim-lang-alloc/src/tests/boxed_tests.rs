use crate::Box;

#[test]
fn box_alloc_and_deref() {
    let x = Box::new(42i32);
    assert_eq!(*x, 42);
}

#[test]
fn box_deref_mut() {
    let mut x = Box::new(10i32);
    *x += 5;
    assert_eq!(*x, 15);
}

#[test]
fn box_large_value() {
    let x = Box::new([0u8; 1024]);
    assert_eq!((*x).len(), 1024);
    assert_eq!((*x)[0], 0);
    assert_eq!((*x)[1023], 0);
}

#[test]
fn box_of_unit() {
    let x = Box::new(());
    assert_eq!(*x, ());
}

#[test]
fn box_drop_releases_memory() {
    // Just verify construction and drop don't panic
    let x = Box::new(42);
    drop(x);
    // If we get here, drop succeeded
}

#[test]
fn box_nested() {
    let inner = Box::new(10);
    let outer = Box::new(inner);
    assert_eq!(**outer, 10);
}
