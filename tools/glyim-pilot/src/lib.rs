pub mod domain_types;
pub mod error;
pub mod metrics;
pub mod process;
pub mod protocol;
pub mod applier;
pub mod config;
pub mod git_ops;
pub mod gates;
pub mod commit;
pub mod session;
pub mod context;
pub mod dispatch;
pub mod server;
pub mod orchestrator;
pub mod cli;

pub use error::PilotError;
pub use domain_types::{ApplyLimits, BannedPattern, DependencyRule};
pub use protocol::types::{FileOp, ParsedOps, PROTOCOL_VERSION};
pub use protocol::parser::{parse_ops_block, extract_ops_blocks};
pub use applier::{
    apply_ops, apply_ops_async, preview_ops, preview_ops_async,
    ApplyResult, ApplyAction, PlannedChange, PlannedAction,
};
