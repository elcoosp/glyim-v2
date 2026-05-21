//! Runtime support: memory allocation, panic handling, drop glue, ABI stubs,
//! networking, threading, and time.

pub use glyim_core::abi::ALIGN_MAX;

use std::alloc::{self, Layout};
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream, UdpSocket, ToSocketAddrs};
use std::sync::{Mutex, OnceLock, Arc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::io::{Read, Write};

// ========== Global Resource Management ==========

type SocketId = u32;
type ThreadId = usize;

struct TcpStreamStore {
    next_id: SocketId,
    streams: HashMap<SocketId, TcpStream>,
}
struct TcpListenerStore {
    next_id: SocketId,
    listeners: HashMap<SocketId, TcpListener>,
}
struct UdpSocketStore {
    next_id: SocketId,
    sockets: HashMap<SocketId, UdpSocket>,
}
struct ThreadInfo {
    handle: JoinHandle<()>,
    thread: Arc<std::thread::Thread>,
}
struct ThreadStore {
    next_id: ThreadId,
    infos: HashMap<ThreadId, ThreadInfo>,
}

fn tcp_streams() -> &'static Mutex<TcpStreamStore> {
    static STREAMS: OnceLock<Mutex<TcpStreamStore>> = OnceLock::new();
    STREAMS.get_or_init(|| Mutex::new(TcpStreamStore { next_id: 1, streams: HashMap::new() }))
}
fn tcp_listeners() -> &'static Mutex<TcpListenerStore> {
    static LISTENERS: OnceLock<Mutex<TcpListenerStore>> = OnceLock::new();
    LISTENERS.get_or_init(|| Mutex::new(TcpListenerStore { next_id: 1, listeners: HashMap::new() }))
}
fn udp_sockets() -> &'static Mutex<UdpSocketStore> {
    static SOCKETS: OnceLock<Mutex<UdpSocketStore>> = OnceLock::new();
    SOCKETS.get_or_init(|| Mutex::new(UdpSocketStore { next_id: 1, sockets: HashMap::new() }))
}
fn threads() -> &'static Mutex<ThreadStore> {
    static THREADS: OnceLock<Mutex<ThreadStore>> = OnceLock::new();
    THREADS.get_or_init(|| Mutex::new(ThreadStore { next_id: 1, infos: HashMap::new() }))
}

// Helper: convert raw bytes to string (assumes valid UTF-8, null-terminated or length provided)
unsafe fn bytes_to_string(ptr: *const u8, len: usize) -> Option<String> {
    if ptr.is_null() { return None; }
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    std::str::from_utf8(slice).ok().map(|s| s.to_string())
}

/// Type for a drop function pointer passed to `glyim_drop_in_place`.
pub type DropFn = unsafe extern "C" fn(*mut u8);

/// Allocate memory with the given size and alignment.
#[unsafe(no_mangle)]
pub extern "C" fn glyim_alloc(size: usize, align: usize) -> *mut u8 {
    if size == 0 {
        return std::ptr::NonNull::dangling().as_ptr();
    }
    let layout = match Layout::from_size_align(size, align.max(1)) {
        Ok(l) => l,
        Err(_) => return std::ptr::null_mut(),
    };
    unsafe { alloc::alloc(layout) }
}

/// Deallocate memory previously allocated by `glyim_alloc`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_dealloc(ptr: *mut u8, size: usize, align: usize) {
    if size == 0 || ptr.is_null() { return; }
    let layout = match Layout::from_size_align(size, align.max(1)) {
        Ok(l) => l,
        Err(_) => return,
    };
    unsafe { alloc::dealloc(ptr, layout) }
}

/// Drop a value in place by calling its type-specific destructor.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_drop_in_place(ptr: *mut u8, drop_fn: Option<DropFn>) {
    if ptr.is_null() { return; }
    if let Some(drop) = drop_fn {
        unsafe { drop(ptr) }
    }
}

/// Panic handler for the runtime.
#[unsafe(no_mangle)]
pub extern "C" fn glyim_panic(_msg: *const u8, _len: usize) -> ! {
    std::process::abort()
}

// ========== Networking (TCP) ==========

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_tcp_connect(
    addr: *const u8,
    addr_len: usize,
    port: u16,
) -> i32 {
    let addr_str = match unsafe { bytes_to_string(addr, addr_len) } {
        Some(s) => s,
        None => return -1,
    };
    let full_addr = format!("{}:{}", addr_str, port);
    let stream = match TcpStream::connect(&full_addr) {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let mut store = tcp_streams().lock().unwrap();
    let id = store.next_id;
    store.next_id += 1;
    store.streams.insert(id, stream);
    id as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_tcp_bind(
    addr: *const u8,
    addr_len: usize,
    port: u16,
) -> i32 {
    let addr_str = match unsafe { bytes_to_string(addr, addr_len) } {
        Some(s) => s,
        None => return -1,
    };
    let full_addr = format!("{}:{}", addr_str, port);
    let listener = match TcpListener::bind(&full_addr) {
        Ok(l) => l,
        Err(_) => return -1,
    };
    let mut store = tcp_listeners().lock().unwrap();
    let id = store.next_id;
    store.next_id += 1;
    store.listeners.insert(id, listener);
    id as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_tcp_accept(fd: i32) -> i32 {
    let fd = fd as u32;
    let mut listener_store = tcp_listeners().lock().unwrap();
    let listener = match listener_store.listeners.get_mut(&fd) {
        Some(l) => l,
        None => return -1,
    };
    let (stream, _) = match listener.accept() {
        Ok(pair) => pair,
        Err(_) => return -1,
    };
    drop(listener_store);
    let mut stream_store = tcp_streams().lock().unwrap();
    let new_id = stream_store.next_id;
    stream_store.next_id += 1;
    stream_store.streams.insert(new_id, stream);
    new_id as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_tcp_read(fd: i32, buf: *mut u8, count: usize) -> isize {
    if buf.is_null() { return -1; }
    let fd = fd as u32;
    let mut store = tcp_streams().lock().unwrap();
    let stream = match store.streams.get_mut(&fd) {
        Some(s) => s,
        None => return -1,
    };
    let slice = unsafe { std::slice::from_raw_parts_mut(buf, count) };
    match stream.read(slice) {
        Ok(n) => n as isize,
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_tcp_write(fd: i32, buf: *const u8, count: usize) -> isize {
    if buf.is_null() { return -1; }
    let fd = fd as u32;
    let mut store = tcp_streams().lock().unwrap();
    let stream = match store.streams.get_mut(&fd) {
        Some(s) => s,
        None => return -1,
    };
    let slice = unsafe { std::slice::from_raw_parts(buf, count) };
    match stream.write_all(slice) {
        Ok(()) => count as isize,
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_tcp_local_addr(
    fd: i32,
    buf: *mut u8,
    buf_len: usize,
) -> i32 {
    let fd = fd as u32;
    let store = tcp_streams().lock().unwrap();
    let stream = match store.streams.get(&fd) {
        Some(s) => s,
        None => return -1,
    };
    let addr = match stream.local_addr() {
        Ok(a) => a,
        Err(_) => return -1,
    };
    let addr_str = addr.to_string();
    let bytes = addr_str.as_bytes();
    if bytes.len() >= buf_len { return -1; }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len());
        *buf.add(bytes.len()) = 0;
    }
    bytes.len() as i32
}

// ========== Networking (UDP) ==========

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_udp_bind(
    addr: *const u8,
    addr_len: usize,
    port: u16,
) -> i32 {
    let addr_str = match unsafe { bytes_to_string(addr, addr_len) } {
        Some(s) => s,
        None => return -1,
    };
    let full_addr = format!("{}:{}", addr_str, port);
    let socket = match UdpSocket::bind(&full_addr) {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let mut store = udp_sockets().lock().unwrap();
    let id = store.next_id;
    store.next_id += 1;
    store.sockets.insert(id, socket);
    id as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_udp_send_to(
    fd: i32,
    buf: *const u8,
    count: usize,
    dest_addr: *const u8,
    dest_addr_len: usize,
    dest_port: u16,
) -> isize {
    if buf.is_null() { return -1; }
    let fd = fd as u32;
    let addr_str = match unsafe { bytes_to_string(dest_addr, dest_addr_len) } {
        Some(s) => s,
        None => return -1,
    };
    let target = match format!("{}:{}", addr_str, dest_port).to_socket_addrs() {
        Ok(mut addrs) => addrs.next(),
        Err(_) => None,
    };
    let target = match target {
        Some(addr) => addr,
        None => return -1,
    };
    let mut store = udp_sockets().lock().unwrap();
    let socket = match store.sockets.get_mut(&fd) {
        Some(s) => s,
        None => return -1,
    };
    let slice = unsafe { std::slice::from_raw_parts(buf, count) };
    match socket.send_to(slice, target) {
        Ok(n) => n as isize,
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_udp_recv_from(
    fd: i32,
    buf: *mut u8,
    count: usize,
    src_addr: *mut u8,
    src_addr_len: *mut usize,
    src_port: *mut u16,
) -> isize {
    if buf.is_null() || src_addr.is_null() || src_addr_len.is_null() || src_port.is_null() {
        return -1;
    }
    let fd = fd as u32;
    let mut store = udp_sockets().lock().unwrap();
    let socket = match store.sockets.get_mut(&fd) {
        Some(s) => s,
        None => return -1,
    };
    let mut slice = unsafe { std::slice::from_raw_parts_mut(buf, count) };
    let (n, addr) = match socket.recv_from(&mut slice) {
        Ok((n, addr)) => (n, addr),
        Err(_) => return -1,
    };
    let addr_str = addr.to_string();
    let parts: Vec<&str> = addr_str.rsplitn(2, ':').collect();
    let (ip, port_val) = if parts.len() == 2 {
        (parts[1], parts[0].parse::<u16>().unwrap_or(0))
    } else {
        ("", 0)
    };
    let ip_bytes = ip.as_bytes();
    let max_len = unsafe { *src_addr_len };
    if ip_bytes.len() >= max_len {
        return -1;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(ip_bytes.as_ptr(), src_addr, ip_bytes.len());
        *src_addr.add(ip_bytes.len()) = 0;
        *src_addr_len = ip_bytes.len() + 1;
        *src_port = port_val;
    }
    n as isize
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_udp_connect(
    fd: i32,
    addr: *const u8,
    addr_len: usize,
    port: u16,
) -> i32 {
    let fd = fd as u32;
    let addr_str = match unsafe { bytes_to_string(addr, addr_len) } {
        Some(s) => s,
        None => return -1,
    };
    let target = match format!("{}:{}", addr_str, port).to_socket_addrs() {
        Ok(mut addrs) => addrs.next(),
        Err(_) => None,
    };
    let target = match target {
        Some(addr) => addr,
        None => return -1,
    };
    let mut store = udp_sockets().lock().unwrap();
    let socket = match store.sockets.get_mut(&fd) {
        Some(s) => s,
        None => return -1,
    };
    match socket.connect(target) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_udp_send(fd: i32, buf: *const u8, count: usize) -> isize {
    if buf.is_null() { return -1; }
    let fd = fd as u32;
    let mut store = udp_sockets().lock().unwrap();
    let socket = match store.sockets.get_mut(&fd) {
        Some(s) => s,
        None => return -1,
    };
    let slice = unsafe { std::slice::from_raw_parts(buf, count) };
    match socket.send(slice) {
        Ok(n) => n as isize,
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_udp_recv(fd: i32, buf: *mut u8, count: usize) -> isize {
    if buf.is_null() { return -1; }
    let fd = fd as u32;
    let mut store = udp_sockets().lock().unwrap();
    let socket = match store.sockets.get_mut(&fd) {
        Some(s) => s,
        None => return -1,
    };
    let slice = unsafe { std::slice::from_raw_parts_mut(buf, count) };
    match socket.recv(slice) {
        Ok(n) => n as isize,
        Err(_) => -1,
    }
}

// ========== Threading ==========

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_thread_spawn(f: extern "C" fn(*mut u8), arg: *mut u8) -> usize {
    let arg_usize = arg as usize;
    let handle = thread::spawn(move || {
        let arg_ptr = arg_usize as *mut u8;
        f(arg_ptr);
    });
    let thread = handle.thread().clone();
    let info = ThreadInfo {
        handle,
        thread: Arc::new(thread),
    };
    let mut store = threads().lock().unwrap();
    let id = store.next_id;
    store.next_id += 1;
    store.infos.insert(id, info);
    id
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_thread_join(handle: usize) -> i32 {
    let handle_id = handle;
    let mut store = threads().lock().unwrap();
    if let Some(info) = store.infos.remove(&handle_id) {
        drop(store);
        match info.handle.join() {
            Ok(()) => 0,
            Err(_) => -1,
        }
    } else {
        -1
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_thread_yield() {
    thread::yield_now();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_thread_sleep(secs: u64, nanos: u32) {
    let duration = Duration::new(secs, nanos);
    thread::sleep(duration);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_thread_park() {
    thread::park();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_thread_unpark(handle: usize) {
    let handle_id = handle;
    let store = threads().lock().unwrap();
    if let Some(info) = store.infos.get(&handle_id) {
        info.thread.unpark();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_thread_current_id() -> usize {
    // Use libc::pthread_self() for a numeric thread ID (Unix).
    #[cfg(unix)]
    {
        use libc::pthread_self;
        unsafe { pthread_self() as usize }
    }
    #[cfg(not(unix))]
    {
        use std::hash::{Hash, Hasher};
        let id = thread::current().id();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        id.hash(&mut hasher);
        hasher.finish() as usize
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_thread_available_parallelism() -> usize {
    match thread::available_parallelism() {
        Ok(n) => n.get(),
        Err(_) => 1,
    }
}

// ========== Time ==========

static START: OnceLock<Instant> = OnceLock::new();

fn monotonic_base() -> &'static Instant {
    START.get_or_init(|| Instant::now())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_time_now_secs() -> u64 {
    monotonic_base().elapsed().as_secs()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_time_now_nanos() -> u64 {
    monotonic_base().elapsed().subsec_nanos() as u64
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_time_system_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_time_system_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
