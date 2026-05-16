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
