use crate::protocol::types::PROTOCOL_VERSION;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ExtensionMessage {
    #[serde(rename = "session.ready", rename_all = "camelCase")]
    SessionReady {
        session_id: String,
        provider_id: String,
        tab_id: u64,
        trace_id: Option<String>,
        v: u32,
    },
    #[serde(rename = "ops.ready", rename_all = "camelCase")]
    OpsReady {
        session_id: String,
        content: String,
        turn: u32,
        trace_id: Option<String>,
        v: u32,
    },
    #[serde(rename = "stream.complete", rename_all = "camelCase")]
    StreamComplete {
        session_id: String,
        turn: u32,
        full_response: String,
        trace_id: Option<String>,
        v: u32,
    },
    #[serde(rename = "error.detected", rename_all = "camelCase")]
    ErrorDetected {
        session_id: String,
        error_type: String,
        error_message: String,
        recoverable: bool,
        trace_id: Option<String>,
        v: u32,
    },
    #[serde(rename = "pong")]
    Pong { timestamp: u64, v: u32 },
}

impl ExtensionMessage {
    pub fn version(&self) -> u32 {
        match self {
            Self::SessionReady { v, .. }
            | Self::OpsReady { v, .. }
            | Self::StreamComplete { v, .. }
            | Self::ErrorDetected { v, .. }
            | Self::Pong { v, .. } => *v,
        }
    }
    pub fn session_id(&self) -> Option<&str> {
        match self {
            Self::SessionReady { session_id, .. }
            | Self::OpsReady { session_id, .. }
            | Self::StreamComplete { session_id, .. }
            | Self::ErrorDetected { session_id, .. } => Some(session_id),
            Self::Pong { .. } => None,
        }
    }
    pub fn trace_id(&self) -> Option<&str> {
        match self {
            Self::SessionReady { trace_id, .. }
            | Self::OpsReady { trace_id, .. }
            | Self::StreamComplete { trace_id, .. }
            | Self::ErrorDetected { trace_id, .. } => trace_id.as_deref(),
            Self::Pong { .. } => None,
        }
    }
    pub fn validate_version(&self) -> Result<(), String> {
        let v = self.version();
        if v == 0 {
            return Err(format!(
                "message with v=0 rejected (current: {})",
                PROTOCOL_VERSION
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CliMessage {
    #[serde(rename = "session.start", rename_all = "camelCase")]
    SessionStart {
        session_id: String,
        provider_id: String,
        prompt: String,
        system_prompt: String,
        trace_id: Option<String>,
        v: u32,
    },
    #[serde(rename = "feedback.send", rename_all = "camelCase")]
    FeedbackSend {
        session_id: String,
        message: String,
        turn: u32,
        trace_id: Option<String>,
        v: u32,
    },
    #[serde(rename = "feedback.continue", rename_all = "camelCase")]
    FeedbackContinue {
        session_id: String,
        trace_id: Option<String>,
        v: u32,
    },
    #[serde(rename = "retry.prompt", rename_all = "camelCase")]
    RetryPrompt {
        session_id: String,
        message: String,
        delay: u64,
        trace_id: Option<String>,
        v: u32,
    },
    #[serde(rename = "session.pause", rename_all = "camelCase")]
    SessionPause {
        session_id: String,
        trace_id: Option<String>,
        v: u32,
    },
    #[serde(rename = "session.abort", rename_all = "camelCase")]
    SessionAbort {
        session_id: String,
        trace_id: Option<String>,
        v: u32,
    },
    #[serde(rename = "ping")]
    Ping { timestamp: u64, v: u32 },
}
