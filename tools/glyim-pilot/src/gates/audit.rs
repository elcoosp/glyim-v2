use crate::error::PilotError;
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct AuditGate;
#[async_trait]
impl Gate for AuditGate {
    fn name(&self) -> &str { "audit" }
    async fn run(&self, _ctx: &GateContext) -> Result<GateResult, PilotError> {
        Ok(GateResult::pass("audit"))
    }
}
