use std::path::{Path, PathBuf};
use url::Url;

pub fn path_to_uri(path: &Path) -> Result<String, String> {
    let url = Url::from_file_path(path).map_err(|_| {
        format!(
            "path is not absolute or cannot be represented as file:// URI: {}",
            path.display()
        )
    })?;
    Ok(url.to_string())
}

pub fn uri_to_file_path(uri: &str) -> Result<PathBuf, String> {
    let url = Url::parse(uri).map_err(|e| format!("invalid URI: {e}"))?;
    url.to_file_path()
        .map_err(|_| format!("not a file:// URI or cannot be converted to path: {uri}"))
}

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
