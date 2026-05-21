//! Time tests for glyim-runtime

use crate::*;
use std::thread;
use std::time::Duration;

#[test]
fn monotonic_time_increases() {
    let t1 = unsafe { glyim_time_now_secs() };
    let n1 = unsafe { glyim_time_now_nanos() };
    thread::sleep(Duration::from_millis(10));
    let t2 = unsafe { glyim_time_now_secs() };
    let n2 = unsafe { glyim_time_now_nanos() };
    // Either seconds increased or nanoseconds increased within same second
    assert!(t2 > t1 || (t2 == t1 && n2 > n1));
}

#[test]
fn system_time_returns_sensible_values() {
    let secs = unsafe { glyim_time_system_secs() };
    let nanos = unsafe { glyim_time_system_nanos() };
    // Should be roughly current time (year 2025 -> > 1700000000)
    assert!(secs > 1_700_000_000);
    assert!(nanos > 0);
}
