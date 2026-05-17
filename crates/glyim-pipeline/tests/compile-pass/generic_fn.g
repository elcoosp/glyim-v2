// test-mode: compile-pass
// compile-flags: --backend mock

fn id<T>(x: T) -> T {
    x
}

fn main() -> i32 {
    let a: i32 = id(42);
    let b: bool = id(true);
    a
}
