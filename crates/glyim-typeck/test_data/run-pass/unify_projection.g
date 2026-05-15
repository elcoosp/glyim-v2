// test-mode: run-pass
fn main() {
    let a: <i32 as Iterator>::Item = 5u32;
    let b: u32 = a;
}
