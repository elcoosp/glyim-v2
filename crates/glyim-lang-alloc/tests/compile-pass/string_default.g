// test-mode: compile-pass
// V17: String::default() works

fn main() {
    let s: String = Default::default();
    assert!(s.is_empty());
    assert_eq!(s.len(), 0);
    assert_eq!(s.as_str(), "");
}
