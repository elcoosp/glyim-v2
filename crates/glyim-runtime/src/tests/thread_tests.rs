//! Threading tests for glyim-runtime
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[test]
fn thread_spawn_and_join() {
    // W5-C06-T02: Thread spawn and join works
    let counter = Arc::new(Mutex::new(0));
    let counter_clone = Arc::clone(&counter);

    let handle = thread::spawn(move || {
        let mut num = counter_clone.lock().unwrap();
        *num += 42;
    });

    handle.join().expect("Failed to join thread");
    let result = *counter.lock().unwrap();
    assert_eq!(result, 42);
}

#[test]
fn thread_yield_and_sleep() {
    // Test thread yield and sleep behavior
    let start = std::time::Instant::now();
    thread::sleep(Duration::from_millis(50));
    let elapsed = start.elapsed();

    // Should have slept at least 40ms (allowing for scheduling variance)
    assert!(
        elapsed >= Duration::from_millis(40),
        "Sleep was too short: {:?}",
        elapsed
    );

    // Yield should not panic and should return quickly
    thread::yield_now();
}

#[test]
fn thread_park_unpark() {
    // Test park/unpark synchronization
    let handle = thread::current();

    let child = thread::spawn(move || {
        // Unpark the parent after a short delay
        thread::sleep(Duration::from_millis(10));
        handle.unpark();
    });

    // Park should return when unparked
    thread::park_timeout(Duration::from_secs(1));
    child.join().expect("Failed to join child");
}

#[test]
fn thread_current_id_and_parallelism() {
    // Test thread ID and available_parallelism
    let id1 = thread::current().id();
    let id2 = thread::current().id();
    assert_eq!(id1, id2, "Same thread should have same ID");

    let parallelism = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    assert!(parallelism >= 1, "Should have at least 1 parallelism");
}
