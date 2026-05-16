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

#[test]
fn string_empty() {
    let s = String::new();
    assert!(s.is_empty());
    assert_eq!(s.len(), 0);
    assert_eq!(s.as_str(), "");
}

#[test]
fn string_long() {
    let mut s = String::new();
    for _ in 0..1000 {
        s.push_str("a");
    }
    assert_eq!(s.len(), 1000);
    assert!(s.as_str().chars().all(|c| c == 'a'));
}

#[test]
fn string_push_str_multiple() {
    let mut s = String::new();
    s.push_str("abc");
    s.push_str("def");
    s.push_str("ghi");
    assert_eq!(s.as_str(), "abcdefghi");
    assert_eq!(s.len(), 9);
}

#[test]
fn string_default() {
    let s: String = Default::default();
    assert!(s.is_empty());
    assert_eq!(s.as_str(), "");
}
