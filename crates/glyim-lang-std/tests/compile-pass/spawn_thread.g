// test-mode: compile-pass
// V18-T03: Spawn a thread and join

fn main() {
    let handle = thread::spawn(|| {
        42
    });
    let result = handle.join();
}
