use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct GateContext {
    pub worktree_dir: PathBuf,
    pub project_root: PathBuf,
    pub default_branch: String,
    pub branch_version: String,
    pub timeout_secs: u64,
    pub changed_files: Vec<String>,
}

impl GateContext {
    pub fn new(
        worktree_dir: PathBuf,
        project_root: PathBuf,
        default_branch: String,
        branch_version: String,
        timeout_secs: u64,
        changed_files: Vec<String>,
    ) -> Self {
        Self {
            worktree_dir,
            project_root,
            default_branch,
            branch_version,
            timeout_secs,
            changed_files,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateSideEffect {
    pub description: String,
    pub affected_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub gate_name: String,
    pub passed: bool,
    pub message: String,
    pub details: Option<String>,
    pub side_effects: Vec<GateSideEffect>,
}

impl GateResult {
    pub fn pass(name: impl Into<String>) -> Self {
        Self {
            gate_name: name.into(),
            passed: true,
            message: "passed".into(),
            details: None,
            side_effects: Vec::new(),
        }
    }
    pub fn pass_with_note(name: impl Into<String>, note: impl Into<String>) -> Self {
        Self {
            gate_name: name.into(),
            passed: true,
            message: note.into(),
            details: None,
            side_effects: Vec::new(),
        }
    }
    pub fn pass_with_side_effects(
        name: impl Into<String>,
        note: impl Into<String>,
        details: impl Into<String>,
        side_effects: Vec<GateSideEffect>,
    ) -> Self {
        Self {
            gate_name: name.into(),
            passed: true,
            message: note.into(),
            details: Some(details.into()),
            side_effects,
        }
    }
    pub fn fail(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            gate_name: name.into(),
            passed: false,
            message: message.into(),
            details: None,
            side_effects: Vec::new(),
        }
    }
    pub fn fail_with_details(
        name: impl Into<String>,
        message: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        Self {
            gate_name: name.into(),
            passed: false,
            message: message.into(),
            details: Some(details.into()),
            side_effects: Vec::new(),
        }
    }
    pub fn has_side_effects(&self) -> bool {
        !self.side_effects.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct PipelineResult {
    pub gates: Vec<GateResult>,
    pub passed: bool,
}

impl PipelineResult {
    pub fn from_gates(gates: Vec<GateResult>) -> Self {
        let passed = gates.iter().all(|g| g.passed);
        Self { gates, passed }
    }
    pub fn first_failure(&self) -> Option<&GateResult> {
        self.gates.iter().find(|g| !g.passed)
    }
    pub fn failure_message(&self) -> String {
        if let Some(fail) = self.first_failure() {
            let mut msg = format!("**{} failed**: {}", fail.gate_name, fail.message);
            if let Some(details) = &fail.details {
                msg = format!("{msg}\n\n```\n{details}\n```");
            }
            let side_effects: Vec<&GateSideEffect> = self
                .gates
                .iter()
                .filter(|g| g.passed && g.has_side_effects())
                .flat_map(|g| &g.side_effects)
                .collect();
            if !side_effects.is_empty() {
                msg.push_str("\n\n**Note: auto-fixes were applied before this failure:**\n");
                for se in side_effects {
                    msg.push_str(&format!("- {}\n", se.description));
                    if !se.affected_files.is_empty() {
                        msg.push_str(&format!("  Files: {}\n", se.affected_files.join(", ")));
                    }
                }
            }
            msg
        } else {
            String::new()
        }
    }
}
