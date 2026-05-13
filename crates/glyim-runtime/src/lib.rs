//! Runtime support (minimal)
pub use glyim_core::abi::ALIGN_MAX;
#[unsafe(no_mangle)]
pub extern "C" fn glyim_alloc(_size: usize, _align: usize) -> *mut u8 { std::ptr::null_mut() }
#[unsafe(no_mangle)]
pub extern "C" fn glyim_dealloc(_ptr: *mut u8, _size: usize, _align: usize) {}
#[unsafe(no_mangle)]
pub extern "C" fn glyim_panic(_msg: *const u8, _len: usize) -> ! { std::process::abort() }
