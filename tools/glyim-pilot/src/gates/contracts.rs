use crate::error::PilotError;
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct ContractGate;
#[async_trait]
impl Gate for ContractGate {
    fn name(&self) -> &str { "contracts" }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        Ok(GateResult::pass("contracts"))
    }
}
