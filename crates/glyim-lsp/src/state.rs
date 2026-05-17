use crate::database::AnalysisDatabase;
use crate::driver::AnalysisMessage;
use glyim_db::Database;
use glyim_diag::GlyimDiagnostic;
use glyim_span::FileId;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub struct LspState {
    pub db: Database,
    open_files: HashMap<PathBuf, (FileId, i32)>,
    analysis: Arc<AnalysisDatabase>,
    driver_tx: Option<tokio::sync::mpsc::Sender<AnalysisMessage>>,
}

impl LspState {
    pub fn new(db: Database) -> Self {
        let analysis = Arc::new(AnalysisDatabase::new());
        Self {
            db,
            open_files: Default::default(),
            analysis,
            driver_tx: None,
        }
    }

    pub fn start_driver(&mut self, cache_dir: PathBuf) {
        if self.driver_tx.is_some() {
            return;
        }
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        let driver = crate::driver::AnalysisDriver::new(self.analysis.clone(), rx, cache_dir);
        tokio::spawn(driver.run());
        self.driver_tx = Some(tx);
    }

    pub fn did_open(&mut self, path: PathBuf, content: String, version: i32) {
        let file_id = self
            .db
            .vfs()
            .add_file_content(&path, Arc::from(content.clone()));
        self.open_files.insert(path.clone(), (file_id, version));
        if let Some(tx) = &self.driver_tx {
            let _ = tx.try_send(AnalysisMessage::FileChanged {
                path,
                content,
                version,
            });
        }
    }

    pub fn did_change(&mut self, path: PathBuf, content: String, version: i32) {
        if let Some(&(file_id, _)) = self.open_files.get(&path) {
            self.db
                .vfs()
                .set_file_content(file_id, Arc::from(content.clone()));
            self.open_files.insert(path.clone(), (file_id, version));
            if let Some(tx) = &self.driver_tx {
                let _ = tx.try_send(AnalysisMessage::FileChanged {
                    path,
                    content,
                    version,
                });
            }
        } else {
            tracing::warn!("did_change for unopened file: {:?}", path);
        }
    }

    pub fn did_close(&mut self, path: &PathBuf) {
        if let Some(tx) = &self.driver_tx {
            let _ = tx.try_send(AnalysisMessage::FileClosed { path: path.clone() });
        }
        self.open_files.remove(path);
    }

    pub fn file_content(&self, path: &PathBuf) -> Option<String> {
        let &(file_id, _) = self.open_files.get(path)?;
        self.db.vfs().file_content(file_id).map(|s| s.to_string())
    }

    pub fn diagnostics_for_file(&self, path: &PathBuf) -> Vec<GlyimDiagnostic> {
        let file_id = self.file_id(path);
        if let Some(file_id) = file_id {
            let guard = self.analysis.diagnostics.read();
            if let Some(lsp_diag) = guard.get(&file_id) {
                return vec![GlyimDiagnostic::internal_error(lsp_diag.message.clone())];
            }
        }
        Vec::new()
    }

    pub fn file_id(&self, path: &PathBuf) -> Option<FileId> {
        self.open_files.get(path).map(|&(id, _)| id)
    }

    #[allow(unused)]
    pub(crate) fn analysis(&self) -> &Arc<AnalysisDatabase> {
        &self.analysis
    }
}
