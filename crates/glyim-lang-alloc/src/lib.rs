//! Glyim Alloc Library – source access only.
//!
//! This crate provides access to the source code of the alloc library
//! written in Glyim. It does not contain Rust implementations of the
//! types; those are defined in the `.g` files.

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
pub fn alloc_source_all() -> String {
    let mut out = String::new();
    for name in alloc_modules() {
        if let Some(src) = alloc_source(name) {
            out.push_str(&format!("// === module: {} ===\n", name));
            out.push_str(src);
            out.push('\n');
        }
    }
    out
}
