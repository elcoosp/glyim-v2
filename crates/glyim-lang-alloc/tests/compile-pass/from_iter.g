// test-mode: compile-pass
// V17-T06: Vec::from_iter works

fn main() {
    let items = [1, 2, 3, 4, 5];
    let v: Vec<i32> = items.into_iter().collect();
    assert_eq!(v.len(), 5);
}
