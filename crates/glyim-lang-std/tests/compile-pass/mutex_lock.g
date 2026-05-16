// test-mode: compile-pass
// V18-T04: Mutex lock and unlock

fn main() {
    let m = Mutex::new(0);
    {
        let guard = m.lock();
        *guard = 42;
    }
    let guard = m.lock();
    assert_eq!(*guard, 42);
}
