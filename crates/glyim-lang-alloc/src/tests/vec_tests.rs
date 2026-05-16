use crate::Vec;

#[test]
fn vec_push_and_index() {
    let mut v: Vec<i32> = Vec::new();
    assert!(v.is_empty());
    v.push(10);
    v.push(20);
    v.push(30);
    assert_eq!(v.len(), 3);
    assert_eq!(v[0], 10);
    assert_eq!(v[1], 20);
    assert_eq!(v[2], 30);
}

#[test]
fn vec_from_iter() {
    let items = [1, 2, 3, 4, 5];
    let v: Vec<i32> = items.into_iter().collect();
    assert_eq!(v.len(), 5);
    assert_eq!(v[0], 1);
    assert_eq!(v[4], 5);
}
