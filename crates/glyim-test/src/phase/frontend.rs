use glyim_span::FileId;
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT_FE_ID: AtomicU32 = AtomicU32::new(2000);

pub struct FrontendTester {
    source: String,
    file_id: FileId,
}

impl FrontendTester {
    pub fn new(source: impl Into<String>) -> Self {
        let file_id = FileId::from_raw(NEXT_FE_ID.fetch_add(1, Ordering::Relaxed));
        Self { source: source.into(), file_id }
    }
    pub fn with_file_id(mut self, id: FileId) -> Self { self.file_id = id; self }
    pub fn run(self) -> super::CompilationTrace {
        let mut trace = super::CompilationTrace::default();
        tracing::info!(phase = "parse", file_id = self.file_id.to_raw());
        let result = glyim_frontend::parse_to_syntax(&self.source, self.file_id);
        trace.parse_diagnostics = result.diagnostics;
        trace.parse_tree = Some(result.root);
        trace
    }
    pub fn parse_only(self) -> glyim_frontend::ParseResult {
        glyim_frontend::parse_to_syntax(&self.source, self.file_id)
    }
}
