//! Glyim Language Standard Library
//!
//! This crate contains the standard library source files for the Glyim language.
//! The actual library code is in `.g` files under `lib/`, written in Glyim syntax.
//! This Rust crate provides access to those source files and testing infrastructure.
//!
//! The std library builds on top of `glyim-lang-core` and provides:
//! - `std::io` — I/O primitives (Read, Write, stdin/stdout/stderr)
//! - `std::fs` — Filesystem operations (File, OpenOptions, directory operations)
//! - `std::net` — Networking (TCP, UDP, IP addresses)
//! - `std::thread` — Native thread spawning and management
//! - `std::sync` — Synchronization primitives (Mutex, RwLock, Arc, atomics)
//! - `std::env` — Environment variables, process arguments, current directory
//! - `std::time` — Time measurement (Duration, Instant, SystemTime)
//! - `std::process` — Child process spawning and management

/// Returns the source code of a standard library module by name.
pub fn std_source(name: &str) -> Option<&'static str> {
    match name {
        "io" => Some(include_str!("../lib/io.g")),
        "fs" => Some(include_str!("../lib/fs.g")),
        "net" => Some(include_str!("../lib/net.g")),
        "thread" => Some(include_str!("../lib/thread.g")),
        "sync" => Some(include_str!("../lib/sync.g")),
        "env" => Some(include_str!("../lib/env.g")),
        "time" => Some(include_str!("../lib/time.g")),
        "process" => Some(include_str!("../lib/process.g")),
        _ => None,
    }
}

/// Returns the names of all standard library modules.
pub fn std_modules() -> &'static [&'static str] {
    &[
        "io", "fs", "net", "thread", "sync", "env", "time", "process",
    ]
}

/// Returns the combined source of all standard library modules.
pub fn std_source_all() -> String {
    let mut out = String::new();
    for name in std_modules() {
        if let Some(src) = std_source(name) {
            out.push_str(&format!("// === module: {} ===\n", name));
            out.push_str(src);
            out.push('\n');
        }
    }
    out
}

/// Returns the total number of standard library modules.
pub fn std_module_count() -> usize {
    std_modules().len()
}

#[cfg(test)]
mod tests;
