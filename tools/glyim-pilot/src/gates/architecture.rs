use crate::domain_types::{DependencyRule, default_architecture_rules};
use crate::error::PilotError;
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct ArchitectureGate { rules: Vec<DependencyRule> }
impl ArchitectureGate { pub fn new(rules: Vec<DependencyRule>) -> Self { Self { rules: if rules.is_empty() { default_architecture_rules() } else { rules } } } }
impl Default for ArchitectureGate { fn default() -> Self { Self::new(default_architecture_rules()) } }

#[async_trait]
impl Gate for ArchitectureGate {
    fn name(&self) -> &str { "architecture" }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        // Simple: check changed Cargo.toml files
        let changed = ctx.changed_files.clone();
        let dir = ctx.worktree_dir.clone();
        let rules = self.rules.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut violations = Vec::new();
            for path in changed {
                if !path.ends_with("Cargo.toml") { continue; }
                let full = dir.join(&path);
                if let Ok(content) = std::fs::read_to_string(full) {
                    // naive: check if dependency present
                    for rule in &rules {
                        if content.contains(&format!("{} =", rule.forbidden_dep)) && content.contains(&format!("name = \"{}\"", rule.from_crate)) {
                            violations.push(format!("{}: {} depends on {} – {}", path, rule.from_crate, rule.forbidden_dep, rule.reason));
                        }
                    }
                }
            }
            if violations.is_empty() { GateResult::pass("architecture") } else { GateResult::fail_with_details("architecture", format!("{} violation(s)", violations.len()), violations.join("\n")) }
        }).await.map_err(|e| PilotError::Gate { gate: "architecture".into(), message: format!("spawn_blocking: {e}") })?;
        Ok(result)
    }
}
