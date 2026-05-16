//! Tests for the structural integrity of std library .g files.

use crate::std_source;

// ============================================================================
// V18-T01: std::io — Write to stdout via println!
// ============================================================================

#[test]
fn io_defines_write_trait() {
    let src = std_source("io").unwrap();
    assert!(src.contains("trait Write"), "io.g should define Write trait");
    assert!(src.contains("fn write"), "io.g should define write method");
    assert!(src.contains("fn flush"), "io.g should define flush method");
    assert!(src.contains("fn write_all"), "io.g should define write_all method");
}

#[test]
fn io_defines_read_trait() {
    let src = std_source("io").unwrap();
    assert!(src.contains("trait Read"), "io.g should define Read trait");
    assert!(src.contains("fn read"), "io.g should define read method");
    assert!(src.contains("fn read_to_end"), "io.g should define read_to_end method");
    assert!(src.contains("fn read_exact"), "io.g should define read_exact method");
}

#[test]
fn io_defines_bufread_trait() {
    let src = std_source("io").unwrap();
    assert!(src.contains("trait BufRead"), "io.g should define BufRead trait");
    assert!(src.contains("fn fill_buf"), "io.g should define fill_buf");
    assert!(src.contains("fn consume"), "io.g should define consume");
    assert!(src.contains("fn read_line"), "io.g should define read_line");
}

#[test]
fn io_defines_std_handles() {
    let src = std_source("io").unwrap();
    assert!(src.contains("struct Stdin"), "io.g should define Stdin struct");
    assert!(src.contains("struct Stdout"), "io.g should define Stdout struct");
    assert!(src.contains("struct Stderr"), "io.g should define Stderr struct");
    assert!(src.contains("fn stdin"), "io.g should define stdin() function");
    assert!(src.contains("fn stdout"), "io.g should define stdout() function");
    assert!(src.contains("fn stderr"), "io.g should define stderr() function");
}

#[test]
fn io_defines_println_macro() {
    let src = std_source("io").unwrap();
    assert!(src.contains("macro println!"), "io.g should define println! macro");
    assert!(src.contains("macro print!"), "io.g should define print! macro");
    assert!(src.contains("macro eprintln!"), "io.g should define eprintln! macro");
    assert!(src.contains("macro eprint!"), "io.g should define eprint! macro");
}

#[test]
fn io_defines_error_types() {
    let src = std_source("io").unwrap();
    assert!(src.contains("struct Error"), "io.g should define Error struct");
    assert!(src.contains("enum ErrorKind"), "io.g should define ErrorKind enum");
    assert!(src.contains("fn last_os_error"), "io.g should define last_os_error");
}

#[test]
fn io_defines_bufreader_bufwriter() {
    let src = std_source("io").unwrap();
    assert!(src.contains("struct BufReader"), "io.g should define BufReader");
    assert!(src.contains("struct BufWriter"), "io.g should define BufWriter");
}

#[test]
fn io_defines_copy_fn() {
    let src = std_source("io").unwrap();
    assert!(src.contains("fn copy"), "io.g should define io::copy function");
}

#[test]
fn io_defines_empty_sink_repeat() {
    let src = std_source("io").unwrap();
    assert!(src.contains("struct Empty"), "io.g should define Empty");
    assert!(src.contains("struct Sink"), "io.g should define Sink");
    assert!(src.contains("struct Repeat"), "io.g should define Repeat");
    assert!(src.contains("fn empty"), "io.g should define empty()");
    assert!(src.contains("fn sink"), "io.g should define sink()");
    assert!(src.contains("fn repeat"), "io.g should define repeat()");
}

#[test]
fn io_uses_extern_c_for_ffi() {
    let src = std_source("io").unwrap();
    assert!(src.contains("extern \"C\""), "io.g should use extern C FFI");
    assert!(src.contains("glyim_stdin_read"), "io.g should reference glyim_stdin_read");
    assert!(src.contains("glyim_stdout_write"), "io.g should reference glyim_stdout_write");
    assert!(src.contains("glyim_stderr_write"), "io.g should reference glyim_stderr_write");
}

// ============================================================================
// V18-T02: std::fs — Read a file from disk
// ============================================================================

#[test]
fn fs_defines_file() {
    let src = std_source("fs").unwrap();
    assert!(src.contains("struct File"), "fs.g should define File struct");
    assert!(src.contains("fn open"), "fs.g should define File::open");
    assert!(src.contains("fn create"), "fs.g should define File::create");
}

#[test]
fn fs_defines_open_options() {
    let src = std_source("fs").unwrap();
    assert!(src.contains("struct OpenOptions"), "fs.g should define OpenOptions struct");
    assert!(src.contains("fn read"), "fs.g should define OpenOptions::read");
    assert!(src.contains("fn write"), "fs.g should define OpenOptions::write");
    assert!(src.contains("fn append"), "fs.g should define OpenOptions::append");
    assert!(src.contains("fn create"), "fs.g should define OpenOptions::create");
}

#[test]
fn fs_file_implements_read_write() {
    let src = std_source("fs").unwrap();
    assert!(src.contains("impl Read for File"), "fs.g should impl Read for File");
    assert!(src.contains("impl Write for File"), "fs.g should impl Write for File");
}

#[test]
fn fs_defines_metadata() {
    let src = std_source("fs").unwrap();
    assert!(src.contains("struct Metadata"), "fs.g should define Metadata struct");
    assert!(src.contains("fn len"), "fs.g should define Metadata::len");
    assert!(src.contains("fn is_file"), "fs.g should define is_file");
    assert!(src.contains("fn is_dir"), "fs.g should define is_dir");
}

#[test]
fn fs_defines_convenience_functions() {
    let src = std_source("fs").unwrap();
    assert!(src.contains("fn read("), "fs.g should define fs::read function");
    assert!(src.contains("fn read_to_string"), "fs.g should define fs::read_to_string");
    assert!(src.contains("fn write("), "fs.g should define fs::write function");
    assert!(src.contains("fn copy("), "fs.g should define fs::copy function");
    assert!(src.contains("fn rename"), "fs.g should define fs::rename function");
}

#[test]
fn fs_defines_directory_operations() {
    let src = std_source("fs").unwrap();
    assert!(src.contains("fn create_dir"), "fs.g should define fs::create_dir");
    assert!(src.contains("fn create_dir_all"), "fs.g should define fs::create_dir_all");
    assert!(src.contains("fn remove_dir"), "fs.g should define fs::remove_dir");
    assert!(src.contains("fn remove_file"), "fs.g should define fs::remove_file");
    assert!(src.contains("fn remove_dir_all"), "fs.g should define fs::remove_dir_all");
}

#[test]
fn fs_defines_exists() {
    let src = std_source("fs").unwrap();
    assert!(src.contains("fn exists"), "fs.g should define fs::exists");
    assert!(src.contains("fn is_dir"), "fs.g should define fs::is_dir");
    assert!(src.contains("fn is_file"), "fs.g should define fs::is_file");
}

#[test]
fn fs_uses_extern_c_for_ffi() {
    let src = std_source("fs").unwrap();
    assert!(src.contains("extern \"C\""), "fs.g should use extern C FFI");
    assert!(src.contains("glyim_fs_open"), "fs.g should reference glyim_fs_open");
    assert!(src.contains("glyim_fs_read"), "fs.g should reference glyim_fs_read");
    assert!(src.contains("glyim_fs_write"), "fs.g should reference glyim_fs_write");
}

// ============================================================================
// std::net — Networking basics
// ============================================================================

#[test]
fn net_defines_ip_addr() {
    let src = std_source("net").unwrap();
    assert!(src.contains("enum IpAddr"), "net.g should define IpAddr enum");
    assert!(src.contains("struct Ipv4Addr"), "net.g should define Ipv4Addr struct");
    assert!(src.contains("struct Ipv6Addr"), "net.g should define Ipv6Addr struct");
}

#[test]
fn net_defines_socket_addr() {
    let src = std_source("net").unwrap();
    assert!(src.contains("enum SocketAddr"), "net.g should define SocketAddr enum");
    assert!(src.contains("struct SocketAddrV4"), "net.g should define SocketAddrV4");
    assert!(src.contains("struct SocketAddrV6"), "net.g should define SocketAddrV6");
}

#[test]
fn net_defines_tcp() {
    let src = std_source("net").unwrap();
    assert!(src.contains("struct TcpStream"), "net.g should define TcpStream");
    assert!(src.contains("struct TcpListener"), "net.g should define TcpListener");
    assert!(src.contains("fn connect"), "net.g should define TcpStream::connect");
    assert!(src.contains("fn bind"), "net.g should define TcpListener::bind");
    assert!(src.contains("fn accept"), "net.g should define TcpListener::accept");
}

#[test]
fn net_defines_udp() {
    let src = std_source("net").unwrap();
    assert!(src.contains("struct UdpSocket"), "net.g should define UdpSocket");
    assert!(src.contains("fn send_to"), "net.g should define UdpSocket::send_to");
    assert!(src.contains("fn recv_from"), "net.g should define UdpSocket::recv_from");
}

#[test]
fn net_tcp_implements_read_write() {
    let src = std_source("net").unwrap();
    assert!(src.contains("impl Read for TcpStream"), "net.g should impl Read for TcpStream");
    assert!(src.contains("impl Write for TcpStream"), "net.g should impl Write for TcpStream");
}

#[test]
fn net_defines_ipv4_helpers() {
    let src = std_source("net").unwrap();
    assert!(src.contains("fn is_loopback"), "net.g should define Ipv4Addr::is_loopback");
    assert!(src.contains("fn is_unspecified"), "net.g should define Ipv4Addr::is_unspecified");
    assert!(src.contains("fn localhost"), "net.g should define Ipv4Addr::localhost");
}

#[test]
fn net_defines_parse_functions() {
    let src = std_source("net").unwrap();
    assert!(src.contains("fn parse_ip_addr"), "net.g should define parse_ip_addr");
    assert!(src.contains("fn parse_socket_addr"), "net.g should define parse_socket_addr");
}

#[test]
fn net_uses_extern_c_for_ffi() {
    let src = std_source("net").unwrap();
    assert!(src.contains("extern \"C\""), "net.g should use extern C FFI");
    assert!(src.contains("glyim_net_tcp_connect"), "net.g should reference glyim_net_tcp_connect");
    assert!(src.contains("glyim_net_tcp_bind"), "net.g should reference glyim_net_tcp_bind");
}

// ============================================================================
// V18-T03: std::thread — Spawn a thread and join
// ============================================================================

#[test]
fn thread_defines_spawn() {
    let src = std_source("thread").unwrap();
    assert!(src.contains("fn spawn"), "thread.g should define spawn function");
}

#[test]
fn thread_defines_join_handle() {
    let src = std_source("thread").unwrap();
    assert!(src.contains("struct JoinHandle"), "thread.g should define JoinHandle");
    assert!(src.contains("fn join"), "thread.g should define JoinHandle::join");
}

#[test]
fn thread_defines_thread_id() {
    let src = std_source("thread").unwrap();
    assert!(src.contains("struct ThreadId"), "thread.g should define ThreadId");
}

#[test]
fn thread_defines_current() {
    let src = std_source("thread").unwrap();
    assert!(src.contains("fn current"), "thread.g should define current() function");
    assert!(src.contains("struct Thread"), "thread.g should define Thread struct");
}

#[test]
fn thread_defines_sleep() {
    let src = std_source("thread").unwrap();
    assert!(src.contains("fn sleep"), "thread.g should define sleep function");
}

#[test]
fn thread_defines_yield_now() {
    let src = std_source("thread").unwrap();
    assert!(src.contains("fn yield_now"), "thread.g should define yield_now function");
}

#[test]
fn thread_defines_builder() {
    let src = std_source("thread").unwrap();
    assert!(src.contains("struct Builder"), "thread.g should define Builder");
    assert!(src.contains("fn name"), "thread.g should define Builder::name");
    assert!(src.contains("fn stack_size"), "thread.g should define Builder::stack_size");
}

#[test]
fn thread_defines_park_unpark() {
    let src = std_source("thread").unwrap();
    assert!(src.contains("fn park"), "thread.g should define park");
    assert!(src.contains("fn unpark"), "thread.g should define unpark");
}

#[test]
fn thread_uses_extern_c_for_ffi() {
    let src = std_source("thread").unwrap();
    assert!(src.contains("extern \"C\""), "thread.g should use extern C FFI");
    assert!(src.contains("glyim_thread_spawn"), "thread.g should reference glyim_thread_spawn");
    assert!(src.contains("glyim_thread_join"), "thread.g should reference glyim_thread_join");
}

#[test]
fn thread_defines_available_parallelism() {
    let src = std_source("thread").unwrap();
    assert!(src.contains("fn available_parallelism"), "thread.g should define available_parallelism");
}

// ============================================================================
// V18-T04: std::sync — Mutex lock and unlock
// ============================================================================

#[test]
fn sync_defines_mutex() {
    let src = std_source("sync").unwrap();
    assert!(src.contains("struct Mutex"), "sync.g should define Mutex struct");
    assert!(src.contains("fn lock"), "sync.g should define Mutex::lock");
    assert!(src.contains("fn new"), "sync.g should define Mutex::new");
}

#[test]
fn sync_defines_mutex_guard() {
    let src = std_source("sync").unwrap();
    assert!(src.contains("struct MutexGuard"), "sync.g should define MutexGuard");
    assert!(src.contains("impl Drop for MutexGuard"), "sync.g should impl Drop for MutexGuard");
}

#[test]
fn sync_mutex_guard_implements_deref() {
    let src = std_source("sync").unwrap();
    assert!(src.contains("impl Deref for MutexGuard"), "sync.g should impl Deref for MutexGuard");
    assert!(src.contains("impl DerefMut for MutexGuard"), "sync.g should impl DerefMut for MutexGuard");
}

#[test]
fn sync_defines_rwlock() {
    let src = std_source("sync").unwrap();
    assert!(src.contains("struct RwLock"), "sync.g should define RwLock");
    assert!(src.contains("fn read"), "sync.g should define RwLock::read");
    assert!(src.contains("fn write"), "sync.g should define RwLock::write");
}

#[test]
fn sync_defines_once() {
    let src = std_source("sync").unwrap();
    assert!(src.contains("struct Once"), "sync.g should define Once");
    assert!(src.contains("fn call_once"), "sync.g should define Once::call_once");
}

#[test]
fn sync_defines_condvar() {
    let src = std_source("sync").unwrap();
    assert!(src.contains("struct Condvar"), "sync.g should define Condvar");
    assert!(src.contains("fn wait"), "sync.g should define Condvar::wait");
    assert!(src.contains("fn notify_one"), "sync.g should define Condvar::notify_one");
    assert!(src.contains("fn notify_all"), "sync.g should define Condvar::notify_all");
}

#[test]
fn sync_defines_barrier() {
    let src = std_source("sync").unwrap();
    assert!(src.contains("struct Barrier"), "sync.g should define Barrier");
    assert!(src.contains("fn wait"), "sync.g should define Barrier::wait");
    assert!(src.contains("BarrierWaitResult"), "sync.g should define BarrierWaitResult");
}

#[test]
fn sync_defines_atomics() {
    let src = std_source("sync").unwrap();
    assert!(src.contains("struct AtomicBool"), "sync.g should define AtomicBool");
    assert!(src.contains("struct AtomicUsize"), "sync.g should define AtomicUsize");
    assert!(src.contains("struct AtomicU8"), "sync.g should define AtomicU8");
    assert!(src.contains("enum Ordering"), "sync.g should define Ordering enum");
}

#[test]
fn sync_defines_arc() {
    let src = std_source("sync").unwrap();
    assert!(src.contains("struct Arc"), "sync.g should define Arc");
    assert!(src.contains("impl Clone for Arc"), "sync.g should impl Clone for Arc");
    assert!(src.contains("impl Drop for Arc"), "sync.g should impl Drop for Arc");
    assert!(src.contains("impl Deref for Arc"), "sync.g should impl Deref for Arc");
}

// ============================================================================
// V18-T05: std::env — Environment variables
// ============================================================================

#[test]
fn env_defines_var() {
    let src = std_source("env").unwrap();
    assert!(src.contains("fn var"), "env.g should define var function");
}

#[test]
fn env_defines_set_var() {
    let src = std_source("env").unwrap();
    assert!(src.contains("fn set_var"), "env.g should define set_var function");
}

#[test]
fn env_defines_remove_var() {
    let src = std_source("env").unwrap();
    assert!(src.contains("fn remove_var"), "env.g should define remove_var function");
}

#[test]
fn env_defines_args() {
    let src = std_source("env").unwrap();
    assert!(src.contains("fn args"), "env.g should define args function");
}

#[test]
fn env_defines_current_dir() {
    let src = std_source("env").unwrap();
    assert!(src.contains("fn current_dir"), "env.g should define current_dir function");
    assert!(src.contains("fn set_current_dir"), "env.g should define set_current_dir function");
}

#[test]
fn env_defines_vars() {
    let src = std_source("env").unwrap();
    assert!(src.contains("fn vars"), "env.g should define vars function");
}

#[test]
fn env_defines_home_dir() {
    let src = std_source("env").unwrap();
    assert!(src.contains("fn home_dir"), "env.g should define home_dir function");
}

#[test]
fn env_defines_temp_dir() {
    let src = std_source("env").unwrap();
    assert!(src.contains("fn temp_dir"), "env.g should define temp_dir function");
}

#[test]
fn env_defines_current_exe() {
    let src = std_source("env").unwrap();
    assert!(src.contains("fn current_exe"), "env.g should define current_exe function");
}

#[test]
fn env_uses_extern_c_for_ffi() {
    let src = std_source("env").unwrap();
    assert!(src.contains("extern \"C\""), "env.g should use extern C FFI");
    assert!(src.contains("glyim_env_var"), "env.g should reference glyim_env_var");
    assert!(src.contains("glyim_env_set_var"), "env.g should reference glyim_env_set_var");
}

// ============================================================================
// std::time — Time measurement
// ============================================================================

#[test]
fn time_defines_duration() {
    let src = std_source("time").unwrap();
    assert!(src.contains("struct Duration"), "time.g should define Duration");
    assert!(src.contains("fn new"), "time.g should define Duration::new");
    assert!(src.contains("fn from_secs"), "time.g should define Duration::from_secs");
    assert!(src.contains("fn from_millis"), "time.g should define Duration::from_millis");
    assert!(src.contains("fn from_micros"), "time.g should define Duration::from_micros");
    assert!(src.contains("fn from_nanos"), "time.g should define Duration::from_nanos");
}

#[test]
fn time_duration_accessors() {
    let src = std_source("time").unwrap();
    assert!(src.contains("fn as_secs"), "time.g should define Duration::as_secs");
    assert!(src.contains("fn subsec_millis"), "time.g should define Duration::subsec_millis");
    assert!(src.contains("fn subsec_micros"), "time.g should define Duration::subsec_micros");
    assert!(src.contains("fn subsec_nanos"), "time.g should define Duration::subsec_nanos");
    assert!(src.contains("fn as_millis"), "time.g should define Duration::as_millis");
    assert!(src.contains("fn as_nanos"), "time.g should define Duration::as_nanos");
}

#[test]
fn time_defines_instant() {
    let src = std_source("time").unwrap();
    assert!(src.contains("struct Instant"), "time.g should define Instant");
    assert!(src.contains("fn now"), "time.g should define Instant::now");
    assert!(src.contains("fn elapsed"), "time.g should define Instant::elapsed");
}

#[test]
fn time_defines_system_time() {
    let src = std_source("time").unwrap();
    assert!(src.contains("struct SystemTime"), "time.g should define SystemTime");
    assert!(src.contains("fn now"), "time.g should define SystemTime::now");
    assert!(src.contains("UNIX_EPOCH"), "time.g should define UNIX_EPOCH");
}

#[test]
fn time_uses_extern_c_for_ffi() {
    let src = std_source("time").unwrap();
    assert!(src.contains("extern \"C\""), "time.g should use extern C FFI");
    assert!(src.contains("glyim_time_now"), "time.g should reference glyim_time_now");
}

// ============================================================================
// std::process — Process management
// ============================================================================

#[test]
fn process_defines_command() {
    let src = std_source("process").unwrap();
    assert!(src.contains("struct Command"), "process.g should define Command");
    assert!(src.contains("fn new"), "process.g should define Command::new");
    assert!(src.contains("fn arg"), "process.g should define Command::arg");
    assert!(src.contains("fn args"), "process.g should define Command::args");
    assert!(src.contains("fn spawn"), "process.g should define Command::spawn");
    assert!(src.contains("fn status"), "process.g should define Command::status");
    assert!(src.contains("fn output"), "process.g should define Command::output");
}

#[test]
fn process_defines_child() {
    let src = std_source("process").unwrap();
    assert!(src.contains("struct Child"), "process.g should define Child");
    assert!(src.contains("fn wait"), "process.g should define Child::wait");
    assert!(src.contains("fn kill"), "process.g should define Child::kill");
    assert!(src.contains("fn id"), "process.g should define Child::id");
}

#[test]
fn process_defines_exit_status() {
    let src = std_source("process").unwrap();
    assert!(src.contains("struct ExitStatus"), "process.g should define ExitStatus");
    assert!(src.contains("fn success"), "process.g should define ExitStatus::success");
    assert!(src.contains("fn code"), "process.g should define ExitStatus::code");
}

#[test]
fn process_defines_stdio() {
    let src = std_source("process").unwrap();
    assert!(src.contains("enum Stdio"), "process.g should define Stdio");
    assert!(src.contains("Inherit"), "process.g should define Stdio::Inherit");
    assert!(src.contains("Piped"), "process.g should define Stdio::Piped");
    assert!(src.contains("Null"), "process.g should define Stdio::Null");
}

#[test]
fn process_defines_exit() {
    let src = std_source("process").unwrap();
    assert!(src.contains("fn exit"), "process.g should define exit function");
}

#[test]
fn process_defines_output() {
    let src = std_source("process").unwrap();
    assert!(src.contains("struct Output"), "process.g should define Output");
    assert!(src.contains("stdout"), "process.g should define Output with stdout");
    assert!(src.contains("stderr"), "process.g should define Output with stderr");
}

#[test]
fn process_uses_extern_c_for_ffi() {
    let src = std_source("process").unwrap();
    assert!(src.contains("extern \"C\""), "process.g should use extern C FFI");
    assert!(src.contains("glyim_process_spawn"), "process.g should reference glyim_process_spawn");
    assert!(src.contains("glyim_process_exit"), "process.g should reference glyim_process_exit");
}
