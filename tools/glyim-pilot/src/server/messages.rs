use serde::{Deserialize, Serialize};
pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ExtensionMessage {
    SessionReady { session_id: String, provider_id: String, tab_id: u64, trace_id: Option<String>, v: u32 },
    OpsReady { session_id: String, content: String, turn: u32, trace_id: Option<String>, v: u32 },
    StreamComplete { session_id: String, turn: u32, full_response: String, trace_id: Option<String>, v: u32 },
    ErrorDetected { session_id: String, error_type: String, error_message: String, recoverable: bool, trace_id: Option<String>, v: u32 },
    Pong { timestamp: u64, v: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CliMessage {
    SessionStart { session_id: String, provider_id: String, prompt: String, system_prompt: String, trace_id: Option<String>, v: u32 },
    FeedbackSend { session_id: String, message: String, turn: u32, trace_id: Option<String>, v: u32 },
    FeedbackContinue { session_id: String, trace_id: Option<String>, v: u32 },
    RetryPrompt { session_id: String, message: String, delay: u64, trace_id: Option<String>, v: u32 },
    SessionPause { session_id: String, trace_id: Option<String>, v: u32 },
    SessionAbort { session_id: String, trace_id: Option<String>, v: u32 },
    Ping { timestamp: u64, v: u32 },
}
