// test-mode: compile-pass
// V17: Box deref_mut allows mutation

fn main() {
    let mut x = Box::new(10);
    *x += 5;
    assert_eq!(*x, 15);
}
