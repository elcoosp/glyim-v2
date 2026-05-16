// test-mode: compile-pass
// compile-flags: --backend=llvm
// check-stdout: 2

fn main() -> i32 {
    let arr = [1, 2, 3];
    arr[1]  // should be 2
}
