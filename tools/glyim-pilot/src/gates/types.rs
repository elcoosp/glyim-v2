use std::path::PathBuf;
#[derive(Debug, Clone)] pub struct GateContext { pub worktree_dir: PathBuf, pub default_branch: String, pub timeout_secs: u64, pub changed_files: Vec<String> }
#[derive(Debug, Clone)] pub struct GateResult { pub gate_name: String, pub passed: bool, pub message: String }
impl GateResult { pub fn pass(name: &str) -> Self { Self { gate_name: name.into(), passed: true, message: "".into() } } }
#[derive(Debug, Clone)] pub struct PipelineResult { pub gates: Vec<GateResult>, pub passed: bool }
impl PipelineResult { pub fn failure_message(&self) -> String { String::new() } }
