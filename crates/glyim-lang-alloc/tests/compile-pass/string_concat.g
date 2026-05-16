// test-mode: compile-pass
// V17-T02: String concatenation works

fn main() {
    let mut s = String::new();
    s.push_str("Hello, ");
    s.push_str("world!");
    assert_eq!(s.as_str(), "Hello, world!");
}
