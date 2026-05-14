use std::path::{Path, PathBuf};

/// Convert a file path to a URI string.
pub fn path_to_uri(path: &Path) -> Result<String, String> {
    tracing::warn!("STUB: path_to_uri");
    Err("not implemented".into())
}

/// Convert a URI string to a file path.
pub fn uri_to_file_path(uri: &str) -> Result<PathBuf, String> {
    tracing::warn!("STUB: uri_to_file_path");
    Err("not implemented".into())
}

/// Convert a byte offset to a (line, character) position given the text.
pub fn offset_to_position(text: &str, offset: usize) -> Result<(usize, usize), String> {
    tracing::warn!("STUB: offset_to_position");
    Err("not implemented".into())
}
