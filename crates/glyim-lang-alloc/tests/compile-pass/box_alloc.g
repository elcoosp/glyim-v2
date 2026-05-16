// test-mode: compile-pass
// V17-T03: Box allocation and deref works

fn main() {
    let x = Box::new(42);
    assert_eq!(*x, 42);
}
