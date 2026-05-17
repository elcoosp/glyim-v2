use crate::error::PilotError;
use std::path::PathBuf;
#[derive(Debug, Clone)] pub enum CommitDecision { Committed { message: String, new_fix_round: u32 }, GateFailed { new_fix_round: u32, feedback: String }, Escalated { new_fix_round: u32, feedback: String } }
pub struct CommitContext { pub worktree_dir: PathBuf, pub stream_id: String, pub commit_message: String, pub current_fix_round: u32, pub timeout_secs: u64, pub default_branch: String, pub branch_version: String, pub changed_files: Vec<String> }
pub struct CommitEngine;
impl CommitEngine { pub async fn evaluate_commit(&self, _: &CommitContext) -> Result<CommitDecision, PilotError> { Ok(CommitDecision::Committed { message: "stub".into(), new_fix_round: 0 }) } pub async fn emergency_commit(&self, _: &CommitContext) -> Result<(), PilotError> { Ok(()) } }
