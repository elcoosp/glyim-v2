//! Threading tests for glyim-runtime

use crate::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn thread_spawn_and_join() {
    let flag = Arc::new(AtomicBool::new(false));
    let flag_ptr = Arc::into_raw(flag.clone()) as *mut u8;
    extern "C" fn thread_func(arg: *mut u8) {
        let flag = unsafe { Arc::from_raw(arg as *const AtomicBool) };
        flag.store(true, Ordering::SeqCst);
        std::mem::forget(flag);
    }
    let handle = unsafe { glyim_thread_spawn(thread_func, flag_ptr) };
    assert_ne!(handle, 0);
    let ret = unsafe { glyim_thread_join(handle) };
    assert_eq!(ret, 0);
    assert!(flag.load(Ordering::SeqCst));
}

#[test]
fn thread_yield_and_sleep() {
    unsafe {
        glyim_thread_yield();
        glyim_thread_sleep(0, 10_000_000);
    }
}

#[test]
fn thread_park_unpark() {
    let flag = Arc::new(AtomicBool::new(false));
    let flag_ptr = Arc::into_raw(flag.clone()) as *mut u8;
    extern "C" fn parked_thread(arg: *mut u8) {
        let flag = unsafe { Arc::from_raw(arg as *const AtomicBool) };
        std::thread::park();
        flag.store(true, Ordering::SeqCst);
        std::mem::forget(flag);
    }
    let handle = unsafe { glyim_thread_spawn(parked_thread, flag_ptr) };
    std::thread::sleep(Duration::from_millis(50));
    unsafe { glyim_thread_unpark(handle) };
    let ret = unsafe { glyim_thread_join(handle) };
    assert_eq!(ret, 0);
    assert!(flag.load(Ordering::SeqCst));
}

#[test]
fn thread_current_id() {
    let id = unsafe { glyim_thread_current_id() };
    assert!(id != 0);
}

#[test]
fn thread_available_parallelism() {
    let n = unsafe { glyim_thread_available_parallelism() };
    assert!(n >= 1);
}
