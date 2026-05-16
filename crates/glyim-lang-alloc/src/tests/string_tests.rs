use crate::String;

#[test]
fn string_concat() {
    let mut s = String::new();
    assert!(s.is_empty());
    s.push_str("Hello, ");
    s.push_str("world!");
    assert_eq!(s.as_str(), "Hello, world!");
    assert_eq!(s.len(), 13);
    assert!(!s.is_empty());
}
