//! File system FFI functions for the Glyim runtime.
//!
//! Provides low-level file operations used by generated code:
//! - File open, close, read, write, flush
//! - Directory creation and removal (single and recursive)
//! - File metadata, rename, truncate, canonicalize
//!
//! # File Descriptor Table
//!
//! Opened files are tracked in a process-global table mapping `i32` file
//! descriptors to `std::fs::File` handles. This allows generated code to
//! reference files by simple integer IDs rather than raw pointers.
//!
//! # Error Codes
//!
//! All functions return negative values on error. See the `FS_E*` constants.
//! Success is indicated by `FS_OK` (0) or a non-negative fd / byte count.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

// ---------------------------------------------------------------------------
// Error codes
// ---------------------------------------------------------------------------

/// Success.
pub const FS_OK: i32 = 0;
/// Generic I/O error.
pub const FS_EIO: i32 = -1;
/// File or path not found.
pub const FS_ENOENT: i32 = -2;
/// Permission denied.
pub const FS_EACCES: i32 = -3;
/// Invalid file descriptor.
pub const FS_EBADF: i32 = -4;
/// Path is not a directory.
pub const FS_ENOTDIR: i32 = -5;
/// Directory is not empty.
pub const FS_ENOTEMPTY: i32 = -6;
/// File already exists.
pub const FS_EEXIST: i32 = -7;
/// Buffer too small.
pub const FS_EOVERFLOW: i32 = -8;

// ---------------------------------------------------------------------------
// Open flags
// ---------------------------------------------------------------------------

/// Open for reading only (default if no access-mode bits are set).
pub const FS_O_RDONLY: u32 = 0;
/// Open for writing only.
pub const FS_O_WRONLY: u32 = 1;
/// Open for reading and writing.
pub const FS_O_RDWR: u32 = 2;
/// Append on each write.
pub const FS_O_APPEND: u32 = 4;
/// Create the file if it does not exist.
pub const FS_O_CREAT: u32 = 8;
/// Truncate the file to zero length if it exists.
pub const FS_O_TRUNC: u32 = 16;

// ---------------------------------------------------------------------------
// File descriptor table
// ---------------------------------------------------------------------------

struct FsTable {
    next_fd: i32,
    files: HashMap<i32, File>,
}

impl FsTable {
    fn new() -> Self {
        Self {
            next_fd: 0,
            files: HashMap::new(),
        }
    }

    /// Insert a file, returning its assigned descriptor.
    fn insert(&mut self, file: File) -> i32 {
        let fd = self.next_fd;
        // INVARIANT: next_fd wraps on overflow; a single process opening
        // 2^31 files is infeasible, so wrapping will not cause collisions
        // with live descriptors in practice.
        self.next_fd = self.next_fd.wrapping_add(1);
        self.files.insert(fd, file);
        fd
    }

    fn get_mut(&mut self, fd: i32) -> Option<&mut File> {
        self.files.get_mut(&fd)
    }

    fn remove(&mut self, fd: i32) -> Option<File> {
        self.files.remove(&fd)
    }
}

static FS_TABLE: OnceLock<Mutex<FsTable>> = OnceLock::new();

fn fs_table() -> &'static Mutex<FsTable> {
    FS_TABLE.get_or_init(|| Mutex::new(FsTable::new()))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a raw byte pointer + length to a `&Path`.
///
/// Returns `None` if the pointer is null or the bytes are not valid UTF-8.
///
/// # Safety
///
/// Caller must ensure `[ptr, ptr + len)` is a valid, readable memory region
/// that remains live for the returned reference's lifetime.
unsafe fn path_from_raw<'a>(ptr: *const u8, len: usize) -> Option<&'a Path> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: Caller guarantees [ptr, ptr + len) is valid readable memory.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    std::str::from_utf8(bytes).ok().map(Path::new)
}

/// Map an `std::io::Error` to a Glyim errno code.
fn io_err_to_errno(err: &std::io::Error) -> i32 {
    match err.kind() {
        std::io::ErrorKind::NotFound => FS_ENOENT,
        std::io::ErrorKind::PermissionDenied => FS_EACCES,
        std::io::ErrorKind::AlreadyExists => FS_EEXIST,
        _ => FS_EIO,
    }
}

// ---------------------------------------------------------------------------
// FFI functions
// ---------------------------------------------------------------------------

/// Open a file and return a file descriptor.
///
/// The `flags` parameter controls the open mode (see `FS_O_*` constants).
/// The lowest two bits encode the access mode: `FS_O_RDONLY` (0), `FS_O_WRONLY`
/// (1), or `FS_O_RDWR` (2). Additional bits for append, create, and truncate
/// may be OR'd in.
///
/// Returns a non-negative file descriptor on success, or a negative error
/// code on failure. The caller must eventually close the fd with
/// `glyim_fs_close` to release the resource.
///
/// # Safety
///
/// - `path` must point to valid UTF-8 data of exactly `path_len` bytes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_fs_open(path: *const u8, path_len: usize, flags: u32) -> i32 {
    let p = match unsafe { path_from_raw(path, path_len) } {
        Some(p) => p,
        None => return FS_EIO,
    };

    let access = flags & 0x3;
    let do_read = access != 1; // anything except WRONLY
    let append = flags & FS_O_APPEND != 0;
    let create = flags & FS_O_CREAT != 0;
    let truncate = flags & FS_O_TRUNC != 0;
    // Write is required for WRONLY, RDWR, and any write-related flag.
    let do_write = access != 0 || append || create || truncate;

    let mut opts = OpenOptions::new();
    opts.read(do_read)
        .write(do_write)
        .append(append)
        .create(create)
        .truncate(truncate);

    match opts.open(p) {
        Ok(file) => {
            let mut table = fs_table().lock().unwrap_or_else(|e| e.into_inner());
            table.insert(file)
        }
        Err(e) => io_err_to_errno(&e),
    }
}

/// Close a file descriptor, releasing the underlying OS handle.
///
/// Returns `FS_OK` on success or `FS_EBADF` if the fd was not open.
///
/// # Safety
///
/// - `fd` must be a value previously returned by `glyim_fs_open` that has
///   not already been closed (double-close returns `FS_EBADF`)
#[unsafe(no_mangle)]
pub extern "C" fn glyim_fs_close(fd: i32) -> i32 {
    let mut table = fs_table().lock().unwrap_or_else(|e| e.into_inner());
    if table.remove(fd).is_some() {
        FS_OK
    } else {
        FS_EBADF
    }
}

/// Read up to `buf_len` bytes from the file into `buf`.
///
/// Returns the number of bytes read (0 indicates EOF), or a negative error
/// code on failure.
///
/// # Safety
///
/// - `fd` must be a valid file descriptor opened for reading
/// - `buf` must point to a writable buffer of at least `buf_len` bytes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_fs_read(fd: i32, buf: *mut u8, buf_len: usize) -> isize {
    if buf.is_null() {
        return FS_EIO as isize;
    }
    // SAFETY: Caller guarantees buf points to writable memory of buf_len bytes.
    let slice = unsafe { std::slice::from_raw_parts_mut(buf, buf_len) };
    let mut table = fs_table().lock().unwrap_or_else(|e| e.into_inner());
    let file = match table.get_mut(fd) {
        Some(f) => f,
        None => return FS_EBADF as isize,
    };
    match file.read(slice) {
        Ok(n) => isize::try_from(n).unwrap_or(isize::MAX),
        Err(e) => io_err_to_errno(&e) as isize,
    }
}

/// Write `buf_len` bytes from `buf` to the file.
///
/// Returns the number of bytes written, or a negative error code on failure.
///
/// # Safety
///
/// - `fd` must be a valid file descriptor opened for writing
/// - `buf` must point to readable data of at least `buf_len` bytes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_fs_write(fd: i32, buf: *const u8, buf_len: usize) -> isize {
    if buf.is_null() && buf_len > 0 {
        return FS_EIO as isize;
    }
    // SAFETY: Caller guarantees buf points to readable memory of buf_len bytes.
    let slice = unsafe { std::slice::from_raw_parts(buf, buf_len) };
    let mut table = fs_table().lock().unwrap_or_else(|e| e.into_inner());
    let file = match table.get_mut(fd) {
        Some(f) => f,
        None => return FS_EBADF as isize,
    };
    match file.write(slice) {
        Ok(n) => isize::try_from(n).unwrap_or(isize::MAX),
        Err(e) => io_err_to_errno(&e) as isize,
    }
}

/// Flush pending writes to the file.
///
/// Returns `FS_OK` on success or a negative error code.
///
/// # Safety
///
/// - `fd` must be a valid file descriptor opened for writing
#[unsafe(no_mangle)]
pub extern "C" fn glyim_fs_flush(fd: i32) -> i32 {
    let mut table = fs_table().lock().unwrap_or_else(|e| e.into_inner());
    let file = match table.get_mut(fd) {
        Some(f) => f,
        None => return FS_EBADF,
    };
    match file.flush() {
        Ok(()) => FS_OK,
        Err(e) => io_err_to_errno(&e),
    }
}

/// Get file metadata (size).
///
/// On success, writes the file size in bytes to `*out_size` and returns
/// `FS_OK`. On failure, returns a negative error code and does not modify
/// `*out_size`.
///
/// # Safety
///
/// - `path` must point to valid UTF-8 data of exactly `path_len` bytes
/// - `out_size` must point to a valid, aligned `u64` writable location
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_fs_metadata(
    path: *const u8,
    path_len: usize,
    out_size: *mut u64,
) -> i32 {
    if out_size.is_null() {
        return FS_EIO;
    }
    let p = match unsafe { path_from_raw(path, path_len) } {
        Some(p) => p,
        None => return FS_EIO,
    };
    match fs::metadata(p) {
        Ok(meta) => {
            // SAFETY: Caller guarantees out_size points to a valid u64.
            unsafe {
                *out_size = meta.len();
            }
            FS_OK
        }
        Err(e) => io_err_to_errno(&e),
    }
}

/// Truncate the file at `path` to the given `size` in bytes.
///
/// Returns `FS_OK` on success or a negative error code.
///
/// # Safety
///
/// - `path` must point to valid UTF-8 data of exactly `path_len` bytes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_fs_truncate(path: *const u8, path_len: usize, size: u64) -> i32 {
    let p = match unsafe { path_from_raw(path, path_len) } {
        Some(p) => p,
        None => return FS_EIO,
    };
    match fs::File::options().write(true).open(p) {
        Ok(file) => match file.set_len(size) {
            Ok(()) => FS_OK,
            Err(e) => io_err_to_errno(&e),
        },
        Err(e) => io_err_to_errno(&e),
    }
}

/// Rename a file or directory from `old_path` to `new_path`.
///
/// Returns `FS_OK` on success or a negative error code.
///
/// # Safety
///
/// - Both path pointers must point to valid UTF-8 data of the given lengths
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_fs_rename(
    old_path: *const u8,
    old_len: usize,
    new_path: *const u8,
    new_len: usize,
) -> i32 {
    let old = match unsafe { path_from_raw(old_path, old_len) } {
        Some(p) => p,
        None => return FS_EIO,
    };
    let new = match unsafe { path_from_raw(new_path, new_len) } {
        Some(p) => p,
        None => return FS_EIO,
    };
    match fs::rename(old, new) {
        Ok(()) => FS_OK,
        Err(e) => io_err_to_errno(&e),
    }
}

/// Remove a file at the given path.
///
/// Returns `FS_OK` on success or a negative error code.
///
/// # Safety
///
/// - `path` must point to valid UTF-8 data of exactly `path_len` bytes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_fs_remove_file(path: *const u8, path_len: usize) -> i32 {
    let p = match unsafe { path_from_raw(path, path_len) } {
        Some(p) => p,
        None => return FS_EIO,
    };
    match fs::remove_file(p) {
        Ok(()) => FS_OK,
        Err(e) => io_err_to_errno(&e),
    }
}

/// Remove an empty directory at the given path.
///
/// Returns `FS_OK` on success or a negative error code.
///
/// # Safety
///
/// - `path` must point to valid UTF-8 data of exactly `path_len` bytes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_fs_remove_dir(path: *const u8, path_len: usize) -> i32 {
    let p = match unsafe { path_from_raw(path, path_len) } {
        Some(p) => p,
        None => return FS_EIO,
    };
    match fs::remove_dir(p) {
        Ok(()) => FS_OK,
        Err(e) => io_err_to_errno(&e),
    }
}

/// Remove a directory and all of its contents recursively.
///
/// Returns `FS_OK` on success or a negative error code.
///
/// # Safety
///
/// - `path` must point to valid UTF-8 data of exactly `path_len` bytes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_fs_remove_dir_all(path: *const u8, path_len: usize) -> i32 {
    let p = match unsafe { path_from_raw(path, path_len) } {
        Some(p) => p,
        None => return FS_EIO,
    };
    match fs::remove_dir_all(p) {
        Ok(()) => FS_OK,
        Err(e) => io_err_to_errno(&e),
    }
}

/// Create a directory at the given path.
///
/// Only creates the final component; the parent directory must already exist.
/// Returns `FS_OK` on success or a negative error code.
///
/// # Safety
///
/// - `path` must point to valid UTF-8 data of exactly `path_len` bytes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_fs_create_dir(path: *const u8, path_len: usize) -> i32 {
    let p = match unsafe { path_from_raw(path, path_len) } {
        Some(p) => p,
        None => return FS_EIO,
    };
    match fs::create_dir(p) {
        Ok(()) => FS_OK,
        Err(e) => io_err_to_errno(&e),
    }
}

/// Create a directory and all parent components as needed.
///
/// Returns `FS_OK` on success (including if the directory already exists)
/// or a negative error code.
///
/// # Safety
///
/// - `path` must point to valid UTF-8 data of exactly `path_len` bytes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_fs_create_dir_all(path: *const u8, path_len: usize) -> i32 {
    let p = match unsafe { path_from_raw(path, path_len) } {
        Some(p) => p,
        None => return FS_EIO,
    };
    match fs::create_dir_all(p) {
        Ok(()) => FS_OK,
        Err(e) => io_err_to_errno(&e),
    }
}

/// Canonicalize a path, resolving symlinks and `.`/`..` components.
///
/// On success, writes the canonical UTF-8 path into `out_buf` (up to
/// `out_buf_len` bytes, **not** including a null terminator) and returns
/// the number of bytes written. On failure, returns a negative error code.
///
/// # Safety
///
/// - `path` must point to valid UTF-8 data of exactly `path_len` bytes
/// - `out_buf` must point to a writable buffer of at least `out_buf_len` bytes
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_fs_canonicalize(
    path: *const u8,
    path_len: usize,
    out_buf: *mut u8,
    out_buf_len: usize,
) -> isize {
    if out_buf.is_null() && out_buf_len > 0 {
        return FS_EIO as isize;
    }
    let p = match unsafe { path_from_raw(path, path_len) } {
        Some(p) => p,
        None => return FS_EIO as isize,
    };
    match fs::canonicalize(p) {
        Ok(canonical) => {
            let bytes = match canonical.to_str() {
                Some(s) => s.as_bytes(),
                None => return FS_EIO as isize,
            };
            if bytes.len() > out_buf_len {
                return FS_EOVERFLOW as isize;
            }
            if !bytes.is_empty() {
                // SAFETY: Caller guarantees out_buf is writable and we
                // verified bytes.len() <= out_buf_len above.
                unsafe {
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, bytes.len());
                }
            }
            isize::try_from(bytes.len()).unwrap_or(isize::MAX)
        }
        Err(e) => io_err_to_errno(&e) as isize,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
