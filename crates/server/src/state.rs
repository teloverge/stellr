use std::sync::Arc;

use stellr_core::Model;
use stellr_history::HistoryStore;
use tokio::sync::{Mutex, Notify, watch};

use crate::spaces::SpaceStore;

/// Shared state for the HTTP and WebSocket server.
pub struct AppState {
    pub hub: watch::Sender<Model>,
    pub token: Option<String>,
    pub spaces: Mutex<SpaceStore>,
    pub refresh: Arc<Notify>,
    pub history: HistoryStore,
}
