use crate::error::PilotError;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub enum OrchestratorAction {
    Feedback { session_id: String, message: String, trace_id: Option<String> },
    Continue { session_id: String, trace_id: Option<String> },
    SelfReview { session_id: String, prompt: String, trace_id: Option<String> },
    StreamComplete { session_id: String, pr_url: String, trace_id: Option<String> },
    Escalate { session_id: String, reason: String, trace_id: Option<String> },
    WaitForResponse { session_id: String, trace_id: Option<String> },
}

pub struct TurnContext {
    pub ops_block: String,
    pub session_id: String,
    pub stream_id: String,
    pub worktree_dir: PathBuf,
    pub project_root: PathBuf,
    pub config: Arc<crate::config::types::PilotConfig>,
    pub persistence: Arc<crate::session::persistence::StatePersistence>,
    pub processing: Arc<Mutex<HashSet<String>>>,
    pub turn: u32,
    pub trace_id: String,
    pub metrics: Arc<dyn crate::metrics::Metrics>,
}

pub async fn process_turn_dispatch(ctx: TurnContext) -> Result<OrchestratorAction, PilotError> {
    Ok(OrchestratorAction::WaitForResponse { session_id: ctx.session_id, trace_id: Some(ctx.trace_id) })
}
