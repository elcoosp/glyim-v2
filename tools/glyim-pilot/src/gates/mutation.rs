use crate::error::PilotError;
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct MutationGate { pub min_kill_rate: f64 }
#[async_trait]
impl Gate for MutationGate {
    fn name(&self) -> &str { "mutation" }
    async fn run(&self, _ctx: &GateContext) -> Result<GateResult, PilotError> {
        Ok(GateResult::pass("mutation"))
    }
}
