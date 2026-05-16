// test-mode: run-pass
// compile-flags: --backend=bytecode
// check-stdout: drop 2\ndrop 1

struct WithDrop {
    id: i32
}

impl Drop for WithDrop {
    fn drop(&mut self) {
        println!("drop {}", self.id);
    }
}

fn main() {
    let _a = WithDrop { id: 1 };
    let _b = WithDrop { id: 2 };
}
