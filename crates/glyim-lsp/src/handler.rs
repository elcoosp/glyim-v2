use crate::AnalysisDatabase;
use crate::driver::AnalysisMessage;
use async_lsp::router::Router;
use std::sync::Arc;
use tokio::sync::mpsc;

pub fn build_router(
    _db: Arc<AnalysisDatabase>,
    _analysis_tx: mpsc::Sender<AnalysisMessage>,
    _client: async_lsp::ClientSocket,
) -> Router<()> {
    Router::new(())
}
