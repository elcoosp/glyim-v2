use super::types::{GateContext, GateResult};
pub async fn run_fmt_fix(_: &GateContext) -> Result<GateResult, crate::error::PilotError> { Ok(GateResult::pass("fmt_fix")) }
