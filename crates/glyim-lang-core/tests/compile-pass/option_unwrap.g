// test-mode: compile-pass
// V16-T01: Option::unwrap works

fn main() {
    let val: Option<i32> = Option::Some(42);
    let unwrapped = val.unwrap();
    assert_eq!(unwrapped, 42);
}
