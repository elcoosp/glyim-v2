use crate::config::PilotConfig;
use std::sync::Arc;
pub async fn run_preflight(_: &Arc<PilotConfig>) { println!("Preflight checks (stub)"); }
