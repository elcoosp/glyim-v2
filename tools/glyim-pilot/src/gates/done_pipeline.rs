use crate::error::PilotError;
use super::types::{GateContext, PipelineResult};
pub async fn run_done_pipeline(_: &GateContext, _: &crate::config::types::ResolvedDoneGates) -> Result<PipelineResult, PilotError> { Ok(PipelineResult { gates: vec![], passed: true }) }
