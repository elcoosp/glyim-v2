// test-mode: compile-pass
// compile-flags: --backend mock

struct MyBox<T> {
    inner: T,
}

fn main() {
    let b = MyBox { inner: 42 };
    // b goes out of scope, drop glue should be generated for MyBox
}
