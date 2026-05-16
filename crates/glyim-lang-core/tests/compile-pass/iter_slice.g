// test-mode: compile-pass
// V16-T02: Iterate over slice using for loop (requires IntoIterator)

fn main() {
    let data: [i32; 3] = [1, 2, 3];
    let mut sum = 0;
    for item in &data {
        sum += *item;
    }
    assert_eq!(sum, 6);
}
