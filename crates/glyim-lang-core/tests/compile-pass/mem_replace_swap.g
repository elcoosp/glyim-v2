// test-mode: compile-pass
// V16-T04: mem::replace, mem::swap

fn main() {
    let mut a = 10;
    let mut b = 20;
    let old = mem::replace(&mut a, 99);
    assert_eq!(old, 10);
    assert_eq!(a, 99);
    mem::swap(&mut a, &mut b);
    assert_eq!(a, 20);
    assert_eq!(b, 99);
}
