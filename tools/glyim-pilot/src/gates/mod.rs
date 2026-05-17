pub mod types;
pub mod helpers;
pub mod fmt_check;
pub mod fmt_fix;
pub mod check;
pub mod clippy;
pub mod banned_pattern;
pub mod architecture;
pub mod contracts;
pub mod commit_pipeline;
pub mod dead_code;
pub mod test;
pub mod coverage;
pub mod mutation;
pub mod workspace_check;
pub mod audit;
pub mod self_review;
pub mod done_pipeline;

use crate::error::PilotError;
use crate::gates::types::GateContext;
use async_trait::async_trait;

pub use types::{GateResult, GateSideEffect, PipelineResult};

#[async_trait]
pub trait Gate: Send + Sync {
    fn name(&self) -> &str;
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError>;
}
