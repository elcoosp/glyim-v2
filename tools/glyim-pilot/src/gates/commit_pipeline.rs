use crate::error::PilotError;
use super::types::{GateContext, PipelineResult};
pub async fn run_commit_pipeline(_: &GateContext, _: &crate::config::types::ResolvedCommitGates, _: Vec<crate::domain_types::BannedPattern>, _: Vec<crate::domain_types::DependencyRule>) -> Result<PipelineResult, PilotError> { Ok(PipelineResult { gates: vec![], passed: true }) }
