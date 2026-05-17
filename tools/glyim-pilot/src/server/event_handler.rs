use crate::orchestrator::OrchestratorAction;
use crate::protocol::types::PROTOCOL_VERSION;
use crate::server::messages::CliMessage;

pub fn map_action_to_cli_message(action: OrchestratorAction, turn: u32) -> Option<CliMessage> {
    match action {
        OrchestratorAction::Feedback { session_id, message, trace_id } => Some(CliMessage::FeedbackSend {
            session_id, message, turn: turn + 1, trace_id, v: PROTOCOL_VERSION,
        }),
        OrchestratorAction::Continue { session_id, trace_id } => Some(CliMessage::FeedbackContinue {
            session_id, trace_id, v: PROTOCOL_VERSION,
        }),
        OrchestratorAction::SelfReview { session_id, prompt, trace_id } => Some(CliMessage::SessionStart {
            session_id, provider_id: "self_review".into(), prompt,
            system_prompt: "You are a code reviewer. Respond with ::APPROVED or fix issues.".into(),
            trace_id, v: PROTOCOL_VERSION,
        }),
        OrchestratorAction::StreamComplete { session_id, pr_url, trace_id } => Some(CliMessage::FeedbackSend {
            session_id, message: format!("Stream complete! PR: {}", pr_url), turn: turn + 1, trace_id, v: PROTOCOL_VERSION,
        }),
        OrchestratorAction::Escalate { session_id, reason, trace_id } => Some(CliMessage::FeedbackSend {
            session_id, message: format!("ESCALATION: {}", reason), turn: turn + 1, trace_id, v: PROTOCOL_VERSION,
        }),
        OrchestratorAction::WaitForResponse { .. } => None,
    }
}
