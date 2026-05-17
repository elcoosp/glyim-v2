use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StreamStatus {
    Init, Seeding, Waiting, Streaming, Executing, Feedback,
    Committing, Committed, Verifying, Reviewing, Complete, Error, Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub session_id: String,
    pub stream_id: String,
    pub provider_id: String,
    pub tab_id: Option<u64>,
    pub status: StreamStatus,
    pub turn: u32,
    pub fix_round: u32,
    pub commits: u32,
    pub worktree_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub error_message: Option<String>,
    pub provider_cooldown_until: Option<DateTime<Utc>>,
}

impl SessionState {
    pub fn new(stream_id: String, provider_id: String, worktree_path: String) -> Self {
        let now = Utc::now();
        Self {
            session_id: uuid::Uuid::new_v4().to_string(), stream_id, provider_id,
            tab_id: None, status: StreamStatus::Init, turn: 0, fix_round: 0, commits: 0,
            worktree_path, created_at: now, updated_at: now, last_activity: now,
            error_message: None, provider_cooldown_until: None,
        }
    }
    pub(crate) fn transition(&mut self, new_status: StreamStatus) {
        let now = Utc::now();
        self.status = new_status;
        self.updated_at = now;
        self.last_activity = now;
    }
    pub(crate) fn record_commit(&mut self) { self.commits += 1; self.fix_round = 0; self.last_activity = Utc::now(); }
    pub(crate) fn record_turn(&mut self) { self.turn += 1; self.last_activity = Utc::now(); }
    pub(crate) fn set_provider_cooldown(&mut self, until: DateTime<Utc>) { self.provider_cooldown_until = Some(until); }
    pub fn is_provider_in_cooldown(&self) -> bool { self.provider_cooldown_until.map_or(false, |until| Utc::now() < until) }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalState {
    pub sessions: HashMap<String, SessionState>,
    pub version: String,
}
impl GlobalState { pub fn new() -> Self { Self { sessions: HashMap::new(), version: env!("CARGO_PKG_VERSION").to_string() } } }
impl Default for GlobalState { fn default() -> Self { Self::new() } }
