use crate::error::PilotError;
use crate::gates::helpers::{is_command_not_found, run_gate_command};
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;
use regex::Regex;
use std::sync::LazyLock;

static COVERAGE_PCT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+\.?\d*)%\s*coverage").unwrap());

pub struct CoverageGate {
    pub min_coverage: f64,
}

#[async_trait]
impl Gate for CoverageGate {
    fn name(&self) -> &str {
        "coverage"
    }

    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        // Run cargo llvm-cov --summary-only
        let output = run_gate_command(
            "cargo",
            &["llvm-cov", "--summary-only"],
            &ctx.worktree_dir,
            ctx.timeout_secs,
            "coverage",
        ).await;

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if let Some(caps) = COVERAGE_PCT_RE.captures(&stdout) {
                    if let Some(pct_str) = caps.get(1) {
                        if let Ok(pct) = pct_str.as_str().parse::<f64>() {
                            if pct >= self.min_coverage {
                                Ok(GateResult::pass_with_note("coverage", format!("Coverage {:.1}% meets minimum {}%", pct, self.min_coverage)))
                            } else {
                                Ok(GateResult::fail_with_details("coverage", format!("Coverage {:.1}% < {}%", pct, self.min_coverage), stdout.to_string()))
                            }
                        } else {
                            Ok(GateResult::fail("coverage", "Failed to parse coverage percentage"))
                        }
                    } else {
                        Ok(GateResult::fail("coverage", "Could not find coverage percentage in output"))
                    }
                } else {
                    Ok(GateResult::fail("coverage", "No coverage data found. Run tests with coverage first."))
                }
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let stdout = String::from_utf8_lossy(&out.stdout);
                if is_command_not_found(&stdout, &stderr) {
                    Err(PilotError::Gate {
                        gate: "coverage".into(),
                        message: "cargo-llvm-cov not installed. Install with: cargo install cargo-llvm-cov".into(),
                    })
                } else {
                    Ok(GateResult::fail_with_details("coverage", "cargo llvm-cov failed", format!("{}{}", stdout, stderr)))
                }
            }
            Err(e) => Err(e),
        }
    }
}
