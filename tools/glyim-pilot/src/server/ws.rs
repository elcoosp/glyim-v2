use crate::error::PilotError;
use crate::server::messages::ExtensionMessage;
use tokio::sync::{broadcast, mpsc};
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub enum ServerEvent {
    Connected { addr: SocketAddr },
    Message { session_id: Option<String>, trace_id: Option<String>, msg: ExtensionMessage },
    Disconnected { addr: SocketAddr },
}

pub struct WsServer {
    pub addr: SocketAddr,
    event_tx: mpsc::Sender<ServerEvent>,
    event_rx: Option<mpsc::Receiver<ServerEvent>>,
    cli_msg_tx: broadcast::Sender<String>,
}

impl WsServer {
    pub fn new(host: &str, port: u16) -> Self {
        let addr = format!("{host}:{port}").parse().expect("invalid bind address");
        let (event_tx, event_rx) = mpsc::channel(1024);
        let (cli_msg_tx, _) = broadcast::channel(256);
        Self { addr, event_tx, event_rx: Some(event_rx), cli_msg_tx }
    }
    pub fn take_event_rx(&mut self) -> Option<mpsc::Receiver<ServerEvent>> { self.event_rx.take() }
    pub fn cli_msg_sender(&self) -> broadcast::Sender<String> { self.cli_msg_tx.clone() }
    pub async fn run(&self) -> Result<(), PilotError> { Ok(()) }
}
