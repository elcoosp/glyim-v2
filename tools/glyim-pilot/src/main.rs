use clap::{Parser, Subcommand};
use glyim_pilot::cli::{render_status_table, run_preflight};
use glyim_pilot::config::{self, PilotConfig};
use glyim_pilot::metrics::production_metrics;
use glyim_pilot::protocol::types::PROTOCOL_VERSION;
use glyim_pilot::server::{CliMessage, ExtensionMessage, ServerEvent, WsServer};
use glyim_pilot::session::persistence::StatePersistence;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Parser)]
#[command(name = "glyim-pilot", version = "0.3.0")]
struct Cli {
    #[arg(long, env = "GLYIM_PROJECT_ROOT", default_value = ".")]
    project_root: PathBuf,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Serve,
    Status,
    Preflight,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let config = match config::load_config(&cli.project_root) {
        Ok(c) => Arc::new(c),
        Err(e) => {
            eprintln!("Config error: {e}");
            std::process::exit(1);
        }
    };

    match cli.command {
        Commands::Serve => run_serve(config, cli.project_root).await,
        Commands::Status => run_status(cli.project_root).await,
        Commands::Preflight => run_preflight(&config).await,
    }
}

async fn run_serve(config: Arc<PilotConfig>, project_root: PathBuf) {
    let mut server = WsServer::new(&config.server.host, config.server.port);
    let mut event_rx = server.take_event_rx().expect("event rx already taken");
    let cli_sender = server.cli_msg_sender();
    let server = Arc::new(server);
    let server_clone = Arc::clone(&server);
    tokio::spawn(async move {
        if let Err(e) = server_clone.run().await {
            tracing::error!("Server error: {e}");
        }
    });

    let persistence = Arc::new(
        StatePersistence::load(&project_root)
            .await
            .expect("failed to load state"),
    );
    let processing: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let metrics: Arc<dyn glyim_pilot::metrics::Metrics> = production_metrics().into();

    tracing::info!(
        "Glym Pilot server started on ws://{}:{}",
        config.server.host,
        config.server.port
    );

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => { tracing::info!("Shutting down..."); break; }
            Some(event) = event_rx.recv() => {
                match event {
                    ServerEvent::Connected { addr } => tracing::info!(peer = %addr, "extension connected"),
                    ServerEvent::Disconnected { addr } => tracing::info!(peer = %addr, "extension disconnected"),
                    ServerEvent::Message { msg, .. } => {
                        handle_extension_message(
                            msg, &config, &persistence, &project_root,
                            &cli_sender, &processing, &metrics,
                        ).await;
                    }
                }
            }
        }
    }
}

async fn handle_extension_message(
    msg: ExtensionMessage,
    config: &Arc<PilotConfig>,
    persistence: &Arc<StatePersistence>,
    project_root: &Path,
    cli_sender: &tokio::sync::broadcast::Sender<String>,
    processing: &Arc<Mutex<HashSet<String>>>,
    metrics: &Arc<dyn glyim_pilot::metrics::Metrics>,
) {
    match msg {
        ExtensionMessage::SessionReady {
            session_id,
            provider_id,
            tab_id,
            ..
        } => {
            tracing::info!(session_id, provider_id, tab_id, "session ready");
        }
        ExtensionMessage::OpsReady {
            session_id,
            content,
            turn,
            trace_id,
            ..
        } => {
            let trace_id = trace_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            let worktree_path = persistence.get_worktree_path(&session_id).await;
            let worktree_dir = match worktree_path {
                Some(path) => PathBuf::from(path),
                None => {
                    tracing::error!(session_id, "worktree_path not found");
                    let err_msg = CliMessage::FeedbackSend {
                        session_id: session_id.clone(),
                        message: "Internal error: worktree path not found".into(),
                        turn: turn + 1,
                        trace_id: Some(trace_id),
                        v: PROTOCOL_VERSION,
                    };
                    let _ = cli_sender.send(serde_json::to_string(&err_msg).unwrap());
                    return;
                }
            };

            let stream_id = persistence
                .get_stream_id(&session_id)
                .await
                .unwrap_or_else(|| session_id.clone());

            let turn_ctx = glyim_pilot::orchestrator::TurnContext {
                ops_block: content,
                session_id,
                stream_id,
                worktree_dir,
                project_root: project_root.to_path_buf(),
                config: Arc::clone(config),
                persistence: Arc::clone(persistence),
                processing: Arc::clone(processing),
                turn,
                trace_id,
                metrics: Arc::clone(metrics),
            };

            let cli_sender_clone = cli_sender.clone();
            let metrics_clone = Arc::clone(metrics);

            tokio::spawn(async move {
                metrics_clone.increment_counter("ops_ready_received", &[]);

                match glyim_pilot::orchestrator::process_turn_dispatch(turn_ctx).await {
                    Ok(action) => {
                        if let Some(cli_msg) =
                            glyim_pilot::server::event_handler::map_action_to_cli_message(
                                action, turn,
                            )
                        {
                            let json = serde_json::to_string(&cli_msg).unwrap();
                            if let Err(e) = cli_sender_clone.send(json) {
                                tracing::warn!("failed to send CLI message: {e}");
                            }
                        } else {
                            tracing::debug!(
                                "orchestrator waiting for response — no CLI message needed"
                            );
                        }
                    }
                    Err(e) => {
                        tracing::error!(?e, "orchestrator error");
                        metrics_clone
                            .increment_counter("orchestrator_error", &[("code", e.code())]);
                    }
                }
            });
        }
        ExtensionMessage::StreamComplete {
            session_id, turn, ..
        } => {
            tracing::info!(session_id, turn, "stream complete");
            metrics.increment_counter("stream_complete", &[]);
        }
        ExtensionMessage::ErrorDetected {
            session_id,
            error_type,
            error_message,
            recoverable,
            trace_id,
            ..
        } => {
            tracing::warn!(
                session_id,
                error_type,
                error_message,
                recoverable,
                "error from extension"
            );
            metrics.increment_counter("extension_error", &[("type", &error_type)]);
            if recoverable {
                let response = CliMessage::FeedbackSend {
                    session_id: session_id.clone(),
                    message: format!("Provider error: {}", error_message),
                    turn: 0,
                    trace_id,
                    v: PROTOCOL_VERSION,
                };
                let _ = cli_sender.send(serde_json::to_string(&response).unwrap());
            }
        }
        ExtensionMessage::Pong { timestamp, .. } => {
            tracing::debug!(timestamp, "pong");
        }
    }
}

async fn run_status(project_root: PathBuf) {
    let persistence = StatePersistence::load(&project_root)
        .await
        .expect("failed to load state");
    let sessions = persistence.all_sessions().await;
    if sessions.is_empty() {
        println!("No sessions found.");
    } else {
        println!("{}", render_status_table(&sessions));
    }
}
