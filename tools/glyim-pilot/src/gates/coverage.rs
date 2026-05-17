use crate::error::PilotError;
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct CoverageGate { pub min_coverage: f64 }
#[async_trait]
impl Gate for CoverageGate {
    fn name(&self) -> &str { "coverage" }
    async fn run(&self, _ctx: &GateContext) -> Result<GateResult, PilotError> {
        Ok(GateResult::pass("coverage"))
    }
}
