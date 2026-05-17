use crate::domain_types::{BannedPattern, default_banned_patterns};
use crate::error::PilotError;
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct BannedPatternGate { patterns: Vec<BannedPattern> }

impl BannedPatternGate {
    pub fn new(patterns: Vec<BannedPattern>) -> Self {
        Self { patterns: if patterns.is_empty() { default_banned_patterns() } else { patterns } }
    }
    pub fn with_defaults() -> Self { Self::new(default_banned_patterns()) }
}
impl Default for BannedPatternGate { fn default() -> Self { Self::with_defaults() } }

#[async_trait]
impl Gate for BannedPatternGate {
    fn name(&self) -> &str { "banned_patterns" }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let changed_files = ctx.changed_files.clone();
        let use_diff = !changed_files.is_empty();
        let dir = ctx.worktree_dir.clone();
        let patterns = self.patterns.clone();

        let result = tokio::task::spawn_blocking(move || {
            let mut violations = Vec::new();
            if use_diff {
                for rel_path in &changed_files {
                    if !rel_path.ends_with(".rs") { continue; }
                    let path = dir.join(rel_path);
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        // simple check: look for pattern in lines (skip comments)
                        for (i, line) in content.lines().enumerate() {
                            if line.trim().starts_with("//") { continue; }
                            for pat in &patterns {
                                if line.contains(&pat.pattern) {
                                    violations.push(format!("{}:{}: {}", rel_path, i+1, pat.description));
                                }
                            }
                        }
                    }
                }
            }
            if violations.is_empty() { GateResult::pass("banned_patterns") }
            else { GateResult::fail_with_details("banned_patterns", format!("{} banned pattern(s) found", violations.len()), violations.join("\n")) }
        }).await.map_err(|e| PilotError::Gate { gate: "banned_patterns".into(), message: format!("spawn_blocking: {e}") })?;
        Ok(result)
    }
}
