// test-mode: compile-fail
// error-pattern: cannot resolve projection
fn main() {
    let _x: <i32 as Dummy>::Item = 0;
}
