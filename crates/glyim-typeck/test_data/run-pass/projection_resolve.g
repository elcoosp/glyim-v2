// test-mode: run-pass
fn main() {
    let _x: <i32 as Iterator>::Item = 42u32;
}
