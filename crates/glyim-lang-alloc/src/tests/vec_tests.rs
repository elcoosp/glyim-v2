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

#[test]
fn vec_pop() {
    let mut v = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    assert_eq!(v.pop(), Some(3));
    assert_eq!(v.pop(), Some(2));
    assert_eq!(v.len(), 1);
    assert_eq!(v.pop(), Some(1));
    assert_eq!(v.pop(), None);
    assert!(v.is_empty());
}

#[test]
fn vec_as_slice() {
    let mut v = Vec::new();
    v.push(10);
    v.push(20);
    v.push(30);
    let s = v.as_slice();
    assert_eq!(s, &[10, 20, 30]);
}

#[test]
fn vec_extend_from_slice() {
    let mut v = Vec::new();
    v.push(1);
    v.extend_from_slice(&[2, 3, 4]);
    assert_eq!(v.len(), 4);
    assert_eq!(v.as_slice(), &[1, 2, 3, 4]);
}

#[test]
fn vec_large_push() {
    let mut v = Vec::new();
    for i in 0..1000 {
        v.push(i);
    }
    assert_eq!(v.len(), 1000);
    assert_eq!(v[0], 0);
    assert_eq!(v[999], 999);
}

#[test]
fn vec_from_iter_empty() {
    let items: [i32; 0] = [];
    let v: Vec<i32> = items.into_iter().collect();
    assert!(v.is_empty());
    assert_eq!(v.len(), 0);
}

#[test]
#[should_panic(expected = "index out of bounds")]
fn vec_index_out_of_bounds() {
    let v: Vec<i32> = Vec::new();
    let _x = v[0];
}

#[test]
fn vec_default_is_empty() {
    let v: Vec<i32> = Default::default();
    assert!(v.is_empty());
    assert_eq!(v.len(), 0);
}
