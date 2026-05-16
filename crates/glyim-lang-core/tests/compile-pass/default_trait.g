// test-mode: compile-pass
// V16-T06: Default trait

fn main() {
    let b: bool = Default::default();
    assert_eq!(b, false);
    let i: i32 = Default::default();
    assert_eq!(i, 0);
    let opt: Option<i32> = Default::default();
    assert!(opt.is_none());
}
