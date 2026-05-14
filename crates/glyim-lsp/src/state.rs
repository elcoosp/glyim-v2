use glyim_db::Database;
use glyim_span::FileId;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub struct LspState {
    pub db: Database,
    /// Maps file path to (FileId, current version)
    open_files: HashMap<PathBuf, (FileId, i32)>,
}

impl LspState {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            open_files: Default::default(),
        }
    }

    /// Handles a didOpen notification. Adds the file to Vfs and stores its version.
    pub fn did_open(&mut self, path: PathBuf, content: String, version: i32) {
        let file_id = self.db.vfs().add_file_content(&path, Arc::from(content));
        self.open_files.insert(path, (file_id, version));
    }

    /// Handles a didChange notification. Updates the file content and version.
    pub fn did_change(&mut self, path: PathBuf, content: String, version: i32) {
        if let Some(&(file_id, _)) = self.open_files.get(&path) {
            self.db.vfs().set_file_content(file_id, Arc::from(content));
            self.open_files.insert(path, (file_id, version));
        } else {
            tracing::warn!("did_change for unopened file: {:?}", path);
        }
    }

    /// Handles a didClose notification. Removes the file from open tracking.
    pub fn did_close(&mut self, path: &PathBuf) {
        self.open_files.remove(path);
    }

    /// Returns the content of an open file, or None if not open.
    pub fn file_content(&self, path: &PathBuf) -> Option<String> {
        let &(file_id, _) = self.open_files.get(path)?;
        self.db.vfs().file_content(file_id).map(|s| s.to_string())
    }

    /// Collect diagnostics for an open file (stub – returns empty for now).
    pub fn diagnostics_for_file(&self, _path: &PathBuf) -> Vec<glyim_diag::GlyimDiagnostic> {
        // Diagnostics will be wired in once the analysis driver is ready.
        Vec::new()
    }

    /// Returns the FileId for an open file.
    pub fn file_id(&self, path: &PathBuf) -> Option<FileId> {
        self.open_files.get(path).map(|&(id, _)| id)
    }
}
