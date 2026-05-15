// test-mode: compile-fail
// error-pattern: trait bound not satisfied
fn main() {
    let _x: <i32 as Iterator>::Item = "hello";
}
