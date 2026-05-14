use glyim_db::Database;
use std::path::PathBuf;

/// Holds the state for the LSP session.
pub struct LspState {
    pub db: Database,
    // Track open documents with their version numbers
    open_files: std::collections::HashMap<PathBuf, i32>,
}

impl LspState {
    pub fn new(db: Database) -> Self {
        tracing::warn!("STUB: LspState::new");
        Self {
            db,
            open_files: Default::default(),
        }
    }

    /// Handle a didOpen notification.
    pub fn did_open(&mut self, path: PathBuf, content: String, version: i32) {
        tracing::warn!("STUB: did_open");
    }

    /// Handle a didChange notification.
    pub fn did_change(&mut self, path: PathBuf, content: String, version: i32) {
        tracing::warn!("STUB: did_change");
    }

    /// Handle a didClose notification.
    pub fn did_close(&mut self, path: &PathBuf) {
        tracing::warn!("STUB: did_close");
    }

    /// Get the current content of an open file.
    pub fn file_content(&self, path: &PathBuf) -> Option<String> {
        tracing::warn!("STUB: file_content");
        None
    }

    /// Publish diagnostics for a file (callback approach).
    pub fn diagnostics_for_file(&self, path: &PathBuf) -> Vec<glyim_diag::GlyimDiagnostic> {
        tracing::warn!("STUB: diagnostics_for_file");
        Vec::new()
    }
}
