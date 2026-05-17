pub mod architecture;
pub mod audit;
pub mod banned_pattern;
pub mod check;
pub mod clippy;
pub mod commit_pipeline;
pub mod contracts;
pub mod coverage;
pub mod dead_code;
pub mod done_pipeline;
pub mod fmt_check;
pub mod fmt_fix;
pub mod helpers;
pub mod mutation;
pub mod self_review;
pub mod test;
pub mod types;
pub mod workspace_check;

use crate::error::PilotError;
use crate::gates::types::GateContext;
use async_trait::async_trait;

pub use types::{GateResult, GateSideEffect, PipelineResult};

#[async_trait]
pub trait Gate: Send + Sync {
    fn name(&self) -> &str;
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError>;
}
