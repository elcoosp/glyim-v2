use std::path::{Path, PathBuf};

/// Convert a filesystem path to a file:// URI.
pub fn path_to_uri(path: &Path) -> Result<String, String> {
    let s = path
        .to_str()
        .ok_or_else(|| format!("path is not valid UTF-8: {}", path.display()))?;
    file_url::file_path_to_url(s)
        .map(|url| url.to_string())
        .map_err(|e| format!("cannot convert path to URI: {e}"))
}

/// Convert a file:// URI string back to a filesystem path.
pub fn uri_to_file_path(uri: &str) -> Result<PathBuf, String> {
    let url = url::Url::parse(uri).map_err(|e| format!("invalid URI: {e}"))?;
    file_url::url_to_path(&url).ok_or_else(|| format!("not a file:// URI: {uri}"))
}

/// Convert a byte offset to (line, column), both 0-based.
pub fn offset_to_position(text: &str, offset: usize) -> Result<(usize, usize), String> {
    if offset > text.len() {
        return Err(format!(
            "offset {offset} out of bounds (len {})",
            text.len()
        ));
    }
    let mut line = 0;
    let mut col = 0;
    for (i, c) in text.char_indices() {
        if i == offset {
            return Ok((line, col));
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += c.len_utf8();
        }
    }
    if offset == text.len() {
        Ok((line, col))
    } else {
        Err("unreachable".into())
    }
}
