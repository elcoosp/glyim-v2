// test-mode: compile-pass

struct Point { x: i32, y: i32 }

fn main() -> i32 {
    let p = Point { x: 10, y: 20 };
    p.x  // field access should compile
}
