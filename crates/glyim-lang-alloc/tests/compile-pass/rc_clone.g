// test-mode: compile-pass
// V17-T04: Rc cloning and reference counting works

fn main() {
    let rc1 = Rc::new(10);
    assert_eq!(Rc::strong_count(&rc1), 1);
    let rc2 = rc1.clone();
    assert_eq!(Rc::strong_count(&rc1), 2);
}
