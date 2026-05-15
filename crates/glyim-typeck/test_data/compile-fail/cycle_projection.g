// test-mode: compile-fail
// error-pattern: cycle detected
fn main() {
    let _: <Self as A>::B = 0;
}
