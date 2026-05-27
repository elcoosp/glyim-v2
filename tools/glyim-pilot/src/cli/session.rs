use clap::Subcommand;
use reqwest::Client;
use serde_json;
use uuid::Uuid;

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
            let client = Client::new();
            let resp = client
                .post("http://127.0.0.1:8421/session/start")
                .json(&serde_json::json!({
                    "provider": provider,
                    "prompt": prompt,
                    "session_id": session_id,
                }))
                .send()
                .await?;
            if resp.status().is_success() {
                println!("Session start sent.");
            } else {
                eprintln!("Failed: {}", resp.text().await?);
            }
            Ok(())
        }
    }
}
