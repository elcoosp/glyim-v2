//! Test function discovery within Glyim source files.
//!
//! Scans `.g` source files for test function declarations using simple
//! text-based pattern matching. Supports `#[test]` attribute detection
//! and `fn test_*` naming conventions.

use std::path::{Path, PathBuf};

/// A single discovered test function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredTest {
    /// Function name (e.g. `test_foo`).
    pub name: String,
    /// Source file containing the test.
    pub file: PathBuf,
    /// 1-based line number where the function is declared.
    pub line: usize,
}

/// Result of scanning a file for test functions.
#[derive(Debug, Clone)]
pub struct FileTestDiscovery {
    /// The source file that was scanned.
    pub file: PathBuf,
    /// Discovered test functions within the file.
    pub tests: Vec<DiscoveredTest>,
}

impl FileTestDiscovery {
    /// Scan a single source file for test functions.
    ///
    /// Detects two patterns:
    /// - `#[test]` attribute followed by `fn <name>(`
    /// - `fn test_<name>(` naming convention
    pub fn scan(file: &Path) -> crate::error::GlyipResult<Self> {
        let content = std::fs::read_to_string(file)?;
        let mut tests = Vec::new();
        let mut pending_test_attr = false;

        for (line_idx, line) in content.lines().enumerate() {
            let line_number = line_idx.saturating_add(1);
            let trimmed = line.trim();

            // Detect #[test] attribute on its own line or inline.
            if trimmed == "#[test]" {
                pending_test_attr = true;
                continue;
            }

            // Detect fn declaration.
            if let Some(fn_name) = extract_fn_name(trimmed) {
                let is_test_fn = pending_test_attr || fn_name.starts_with("test_");
                if is_test_fn {
                    tests.push(DiscoveredTest {
                        name: fn_name.to_string(),
                        file: file.to_path_buf(),
                        line: line_number,
                    });
                }
                pending_test_attr = false;
            } else {
                // If there was a #[test] but the next non-empty line isn't a fn,
                // keep looking (but reset for safety if it's clearly not a fn).
                if !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("//") {
                    pending_test_attr = false;
                }
            }
        }

        Ok(FileTestDiscovery {
            file: file.to_path_buf(),
            tests,
        })
    }

    /// Number of discovered tests in this file.
    pub fn count(&self) -> usize {
        self.tests.len()
    }
}

/// Extract the function name from a line like `fn foo(` or `pub fn foo(`.
fn extract_fn_name(line: &str) -> Option<&str> {
    // Find "fn " then the identifier after it.
    let line = line.strip_prefix("pub ").unwrap_or(line);
    let line = line.trim_start();

    let after_fn = line.strip_prefix("fn ")?;
    let after_fn = after_fn.trim_start();

    // The function name is everything up to '(' or '<' (for generics).
    let name_end = after_fn
        .find(|c: char| c == '(' || c == '<' || c == '{' || c.is_whitespace())
        .unwrap_or(after_fn.len());
    let name = &after_fn[..name_end];

    if name.is_empty() { None } else { Some(name) }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn extract_fn_name_simple() {
        assert_eq!(extract_fn_name("fn foo() {"), Some("foo"));
    }

    #[test]
    fn extract_fn_name_pub() {
        assert_eq!(extract_fn_name("pub fn bar() {"), Some("bar"));
    }

    #[test]
    fn extract_fn_name_with_generics() {
        assert_eq!(extract_fn_name("fn baz<T>(x: T) {"), Some("baz"));
    }

    #[test]
    fn extract_fn_name_not_a_fn() {
        assert_eq!(extract_fn_name("let x = 5;"), None);
    }

    #[test]
    fn extract_fn_name_empty() {
        assert_eq!(extract_fn_name("fn ()"), None);
    }

    #[test]
    fn scan_file_with_test_attr() {
        let dir = tempfile::TempDir::new().unwrap();
        let file = dir.path().join("test_example.g");
        std::fs::write(
            &file,
            "#[test]\nfn test_something() {\n    assert(true);\n}\nfn helper() {}\n",
        )
        .unwrap();

        let discovery = FileTestDiscovery::scan(&file).unwrap();
        assert_eq!(discovery.tests.len(), 1);
        assert_eq!(discovery.tests[0].name, "test_something");
        assert_eq!(discovery.tests[0].line, 2);
    }

    #[test]
    fn scan_file_with_test_naming_convention() {
        let dir = tempfile::TempDir::new().unwrap();
        let file = dir.path().join("naming.g");
        std::fs::write(&file, "fn test_add() {}\nfn test_sub() {}\nfn main() {}\n").unwrap();

        let discovery = FileTestDiscovery::scan(&file).unwrap();
        assert_eq!(discovery.tests.len(), 2);
        assert_eq!(discovery.tests[0].name, "test_add");
        assert_eq!(discovery.tests[1].name, "test_sub");
    }

    #[test]
    fn scan_file_no_tests() {
        let dir = tempfile::TempDir::new().unwrap();
        let file = dir.path().join("no_tests.g");
        std::fs::write(&file, "fn main() {}\nfn helper() {}\n").unwrap();

        let discovery = FileTestDiscovery::scan(&file).unwrap();
        assert!(discovery.tests.is_empty());
    }
}
