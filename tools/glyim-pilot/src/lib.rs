pub mod applier;
pub mod cli;
pub mod commit;
pub mod config;
pub mod context;
pub mod dispatch;
pub mod domain_types;
pub mod error;
pub mod gates;
pub mod git_ops;
pub mod metrics;
pub mod orchestrator;
pub mod process;
pub mod protocol;
pub mod server;
pub mod session;

pub use applier::{
    apply_ops, apply_ops_async, preview_ops, preview_ops_async, ApplyAction, ApplyResult,
    PlannedAction, PlannedChange,
};
pub use domain_types::{ApplyLimits, BannedPattern, DependencyRule};
pub use error::PilotError;
pub use protocol::parser::{extract_ops_blocks, parse_ops_block};
pub use protocol::types::{FileOp, ParsedOps, PROTOCOL_VERSION};
