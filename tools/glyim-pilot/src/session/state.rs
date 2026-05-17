use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamStatus { Init, Seeding, Waiting, Streaming, Executing, Feedback, Committing, Committed, Verifying, Reviewing, Complete, Error, Paused }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub stream_id: String, pub provider_id: String, pub worktree_path: String, pub status: StreamStatus, pub turn: u32, pub fix_round: u32, pub commits: u32,
}
impl SessionState { pub fn new(stream_id: String, provider_id: String, worktree_path: String) -> Self { Self { stream_id, provider_id, worktree_path, status: StreamStatus::Init, turn: 0, fix_round: 0, commits: 0 } } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalState { pub sessions: HashMap<String, SessionState> }
impl GlobalState { pub fn new() -> Self { Self { sessions: HashMap::new() } } }
