use crate::error::PilotError;
use crate::gates::helpers::{is_command_not_found, run_gate_command};
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct AuditGate;

#[async_trait]
impl Gate for AuditGate {
    fn name(&self) -> &str {
        "audit"
    }

    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let output = run_gate_command(
            "cargo",
            &["audit"],
            &ctx.worktree_dir,
            ctx.timeout_secs,
            "audit",
        ).await;

        match output {
            Ok(out) if out.status.success() => {
                Ok(GateResult::pass("audit"))
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let stdout = String::from_utf8_lossy(&out.stdout);
                if is_command_not_found(&stdout, &stderr) {
                    Err(PilotError::Gate {
                        gate: "audit".into(),
                        message: "cargo-audit not installed. Install with: cargo install cargo-audit".into(),
                    })
                } else {
                    Ok(GateResult::fail_with_details("audit", "Security vulnerabilities found", format!("{}{}", stdout, stderr)))
                }
            }
            Err(e) => Err(e),
        }
    }
}
