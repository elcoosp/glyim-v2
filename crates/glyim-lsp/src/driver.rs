use crate::AnalysisDatabase;
use crate::database::SourceMap;
use glyim_core::{CrateId, Interner};
use glyim_def_map::build_def_map;
use glyim_frontend::{lex, parse_to_syntax};
use glyim_hir::pipeline_api::lower_crate_for_pipeline;
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
    #[allow(dead_code)]
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

        let crate_id = CrateId::from_raw(0);

        let lex_result = lex(content, file_id);
        let parse_result = parse_to_syntax(content, file_id);
        let (_def_map, def_diagnostics) = build_def_map(&parse_result.root, crate_id);

        let mut interner = Interner::new();
        let (hir, _hir_diags) = lower_crate_for_pipeline(&parse_result.root, &mut interner);

        self.db
            .symbol_index
            .write()
            .build_from_hir(file_id, &hir, &interner);
        self.db
            .reference_graph
            .write()
            .build_from_hir(file_id, &hir, &interner);
        self.db.hirs.write().insert(file_id, hir);

        let mut all_diagnostics = Vec::new();
        all_diagnostics.extend(lex_result.diagnostics);
        all_diagnostics.extend(parse_result.diagnostics);
        all_diagnostics.extend(def_diagnostics);
        all_diagnostics.extend(_hir_diags);

        let lsp_diagnostics =
            crate::diagnostics::convert_diagnostics(file_id, &sm, &all_diagnostics);
        if lsp_diagnostics.is_empty() {
            self.db.diagnostics.write().remove(&file_id);
        } else {
            for diag in lsp_diagnostics {
                self.db.diagnostics.write().insert(file_id, diag);
            }
        }

        tracing::debug!(
            "Analyzed file {:?} with {} diagnostics",
            path,
            all_diagnostics.len()
        );
    }
}
