use crate::AnalysisDatabase;
use crate::database::SourceMap;
use glyim_span::FileId;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;

pub enum AnalysisMessage {
    FileChanged { path: PathBuf, content: String, version: i32 },
    FileClosed { path: PathBuf },
    Shutdown,
}

pub struct AnalysisDriver {
    db: Arc<AnalysisDatabase>,
    rx: Receiver<AnalysisMessage>,
    cache_dir: PathBuf,
}

impl AnalysisDriver {
    pub fn new(db: Arc<AnalysisDatabase>, rx: Receiver<AnalysisMessage>, cache_dir: PathBuf) -> Self {
        Self { db, rx, cache_dir }
    }

    pub async fn run(mut self) {
        while let Some(msg) = self.rx.recv().await {
            match msg {
                AnalysisMessage::FileChanged { path, content, version: _ } => {
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
        self.db.source_maps.write().insert(file_id, sm);
        tracing::debug!("Analyzed file {:?}", path);
    }
}
