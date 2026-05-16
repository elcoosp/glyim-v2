//! Glyim Alloc Library
//!
//! This crate contains the standard library source files for dynamic allocation
//! and collections in the Glyim language. The actual library code is in `.g`
//! files under `lib/`, written in Glyim syntax. This Rust crate provides access
//! to those source files and testing infrastructure.

pub mod alloc;
pub mod boxed;
pub mod raw_vec;
pub mod rc;
pub mod string;
pub mod vec;

pub use alloc::GlobalAlloc;
pub use boxed::Box;
pub use raw_vec::RawVec;
pub use rc::Rc;
pub use string::String;
pub use vec::Vec;

/// Returns the source code of an alloc library module by name.
pub fn alloc_source(name: &str) -> Option<&'static str> {
    match name {
        "alloc" => Some(include_str!("../lib/alloc.g")),
        "boxed" => Some(include_str!("../lib/boxed.g")),
        "vec" => Some(include_str!("../lib/vec.g")),
        "string" => Some(include_str!("../lib/string.g")),
        "rc" => Some(include_str!("../lib/rc.g")),
        "raw_vec" => Some(include_str!("../lib/raw_vec.g")),
        _ => None,
    }
}

/// Returns the names of all alloc library modules.
pub fn alloc_modules() -> &'static [&'static str] {
    &["alloc", "boxed", "vec", "string", "rc", "raw_vec"]
}

/// Returns the combined source of all alloc library modules.
pub fn alloc_source_all() -> std::string::String {
    let mut out = std::string::String::new();
    for name in alloc_modules() {
        if let Some(src) = alloc_source(name) {
            out.push_str(&format!("// === module: {} ===\n", name));
            out.push_str(src);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests;
