use stellr_core::Model;
use tokio::sync::watch;

/// Shared state for the HTTP and WebSocket server.
pub struct AppState {
    pub hub: watch::Sender<Model>,
    pub token: Option<String>,
}
