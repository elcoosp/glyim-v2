use crate::error::PilotError;
use super::state::{SessionState, GlobalState};
use std::path::Path;
use tokio::sync::Mutex;

pub struct StatePersistence { inner: Mutex<GlobalState> }
impl StatePersistence {
    pub async fn load(_: &Path) -> Result<Self, PilotError> { Ok(Self { inner: Mutex::new(GlobalState::new()) }) }
    pub async fn add_session(&self, s: SessionState) -> Result<(), PilotError> { self.inner.lock().await.sessions.insert(s.stream_id.clone(), s); Ok(()) }
    pub async fn get_worktree_path(&self, stream_id: &str) -> Option<String> { self.inner.lock().await.sessions.get(stream_id).map(|s| s.worktree_path.clone()) }
    pub async fn get_stream_id(&self, session_id: &str) -> Option<String> { Some(session_id.to_string()) }
    pub async fn get_fix_round(&self, stream_id: &str) -> u32 { self.inner.lock().await.sessions.get(stream_id).map(|s| s.fix_round).unwrap_or(0) }
    pub async fn all_sessions(&self) -> Vec<SessionState> { self.inner.lock().await.sessions.values().cloned().collect() }
}
