use clap::Subcommand;
use futures_util::{SinkExt, StreamExt};
use serde_json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use uuid::Uuid;

use crate::protocol::types::PROTOCOL_VERSION;
use crate::server::messages::CliMessage;

#[derive(Subcommand)]
pub enum SessionCommands {
    /// Start a new session with a provider
    Start {
        /// Provider ID (deepseek, gemini, grok, etc.)
        #[arg(short = 'P', long, default_value = "deepseek")]
        provider: String,
        /// Prompt to send
        #[arg(
            short = 'p',
            long,
            default_value = "Write a simple Rust function that adds two numbers"
        )]
        prompt: String,
        /// Optional session ID (auto-generated if not provided)
        #[arg(short, long)]
        session_id: Option<String>,
    },
}

pub async fn handle_session_command(cmd: SessionCommands) -> Result<(), anyhow::Error> {
    match cmd {
        SessionCommands::Start {
            provider,
            prompt,
            session_id,
        } => {
            let session_id = session_id.unwrap_or_else(|| Uuid::new_v4().to_string());
            let msg = CliMessage::SessionStart {
                session_id,
                provider_id: provider,
                prompt,
                system_prompt:
                    "You are a helpful assistant. Output only code inside ```glyim-ops blocks."
                        .into(),
                trace_id: None,
                v: PROTOCOL_VERSION,
            };
            let msg_json = serde_json::to_string(&msg)?;

            let (mut ws, _) = connect_async("ws://127.0.0.1:8420").await?;
            ws.send(Message::Text(msg_json.into())).await?;
            println!("Session start sent. Waiting for response...");

            tokio::time::timeout(tokio::time::Duration::from_secs(5), async {
                if let Some(Ok(Message::Text(resp))) = ws.next().await {
                    println!("Server response: {}", resp);
                } else {
                    println!("No response received (timeout or unexpected message)");
                }
            })
            .await
            .unwrap_or_else(|_| println!("No response within 5 seconds"));

            Ok(())
        }
    }
}
