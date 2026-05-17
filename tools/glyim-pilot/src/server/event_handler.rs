use crate::orchestrator::OrchestratorAction;
use crate::server::messages::CliMessage;

pub fn map_action_to_cli_message(_action: OrchestratorAction, _turn: u32) -> Option<CliMessage> {
    None
}
