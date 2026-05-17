use crate::error::PilotError;
use super::state::{GlobalState, SessionState};
use std::path::Path;
use tokio::sync::Mutex;

const STATE_FILE: &str = ".glyim-pilot-state.json";

struct Inner { path: std::path::PathBuf, state: GlobalState }

impl Inner {
    async fn load(project_root: &Path) -> Result<Self, PilotError> {
        let path = project_root.join(STATE_FILE);
        let state = if path.exists() {
            let content = tokio::fs::read_to_string(&path).await
                .map_err(|e| PilotError::Session(format!("failed to read state: {e}")))?;
            serde_json::from_str(&content)
                .map_err(|e| PilotError::Session(format!("failed to parse state: {e}")))?
        } else { GlobalState::new() };
        Ok(Self { path, state })
    }
    async fn save(&self) -> Result<(), PilotError> {
        let content = serde_json::to_string(&self.state)
            .map_err(|e| PilotError::Session(format!("serialization failed: {e}")))?;
        let tmp_path = std::path::PathBuf::from(format!("{}.tmp", self.path.display()));
        tokio::fs::write(&tmp_path, &content).await
            .map_err(|e| PilotError::Session(format!("temp write failed: {e}")))?;
        tokio::fs::rename(&tmp_path, &self.path).await
            .map_err(|e| PilotError::Session(format!("rename failed: {e}")))?;
        Ok(())
    }
}

pub struct StatePersistence { inner: Mutex<Inner> }

impl StatePersistence {
    pub async fn load(project_root: &Path) -> Result<Self, PilotError> {
        let inner = Inner::load(project_root).await?;
        Ok(Self { inner: Mutex::new(inner) })
    }
    pub async fn add_session(&self, session: SessionState) -> Result<(), PilotError> {
        let mut p = self.inner.lock().await;
        p.state.sessions.insert(session.stream_id.clone(), session);
        p.save().await
    }
    pub async fn try_update_session<F>(&self, stream_id: &str, f: F) -> Result<(), PilotError>
    where F: FnOnce(&mut SessionState) -> Result<(), PilotError> {
        let mut p = self.inner.lock().await;
        let session = p.state.sessions.get_mut(stream_id)
            .ok_or_else(|| PilotError::Session(format!("session {stream_id} not found")))?;
        let backup = session.clone();
        if let Err(e) = f(session) {
            *p.state.sessions.get_mut(stream_id).unwrap() = backup;
            return Err(e);
        }
        p.save().await
    }
    pub async fn get_worktree_path(&self, stream_id: &str) -> Option<String> {
        self.inner.lock().await.state.sessions.get(stream_id).map(|s| s.worktree_path.clone())
    }
    pub async fn get_stream_id(&self, session_id: &str) -> Option<String> {
        let p = self.inner.lock().await;
        p.state.sessions.values().find(|s| s.session_id == session_id).map(|s| s.stream_id.clone())
    }
    pub async fn get_fix_round(&self, stream_id: &str) -> u32 {
        self.inner.lock().await.state.sessions.get(stream_id).map(|s| s.fix_round).unwrap_or(0)
    }
    pub async fn all_sessions(&self) -> Vec<SessionState> {
        self.inner.lock().await.state.sessions.values().cloned().collect()
    }
}

impl std::fmt::Debug for StatePersistence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StatePersistence").finish_non_exhaustive()
    }
}
