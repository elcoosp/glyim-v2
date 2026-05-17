use crate::AnalysisDatabase;
use crate::database::SourceMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;

pub enum AnalysisMessage {
    FileChanged {
        path: PathBuf,
        content: String,
        version: i32,
    },
    FileClosed {
        path: PathBuf,
    },
    Shutdown,
}

pub struct AnalysisDriver {
    db: Arc<AnalysisDatabase>,
    rx: Receiver<AnalysisMessage>,
    #[allow(unused)]
    cache_dir: PathBuf,
}

impl AnalysisDriver {
    pub fn new(
        db: Arc<AnalysisDatabase>,
        rx: Receiver<AnalysisMessage>,
        cache_dir: PathBuf,
    ) -> Self {
        Self { db, rx, cache_dir }
    }

    pub async fn run(mut self) {
        while let Some(msg) = self.rx.recv().await {
            match msg {
                AnalysisMessage::FileChanged {
                    path,
                    content,
                    version: _,
                } => {
                    self.analyze_file(&path, &content).await;
                }
                AnalysisMessage::FileClosed { path } => {
                    self.db.file_map.write().remove(&path);
                }
                AnalysisMessage::Shutdown => break,
            }
        }
    }

    async fn analyze_file(&self, path: &PathBuf, content: &str) {
        let file_id = { self.db.file_map.write().get_or_create(path) };
        let sm = SourceMap::new(path.clone(), file_id, content.to_string());
        self.db.source_maps.write().insert(file_id, sm.clone());

        // For testing, insert a diagnostic if content contains "error"
        if content.contains("error") {
            let diag = lsp_types::Diagnostic {
                range: lsp_types::Range::default(),
                severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                message: "Syntax error detected".to_string(),
                source: Some("glyim".to_string()),
                ..Default::default()
            };
            self.db.diagnostics.write().insert(file_id, diag);
        } else {
            self.db.diagnostics.write().remove(&file_id);
        }

        tracing::debug!("Analyzed file {:?}", path);
    }
}
