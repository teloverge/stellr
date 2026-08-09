use std::{
    io,
    net::SocketAddr,
    num::NonZeroU64,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use stellr_core::{Model, Provider, ProviderError, ProviderSnapshot, RepoRef};
use stellr_github::cache::Cache;
use stellr_server::{
    poll::{PollingControl, spawn_controlled_poller},
    routes::router,
    spaces::SpaceStore,
    state::AppState,
};
use thiserror::Error;
use tokio::{
    sync::{RwLock, watch},
    task::JoinHandle,
};

#[derive(Clone)]
pub struct ProviderSlot {
    current: Arc<RwLock<Arc<dyn Provider + Send + Sync>>>,
    generation: Arc<AtomicU64>,
    confirmed_generation: Arc<AtomicU64>,
    publication: Arc<std::sync::Mutex<()>>,
}

impl ProviderSlot {
    pub fn new(provider: Arc<dyn Provider + Send + Sync>) -> Self {
        Self {
            current: Arc::new(RwLock::new(provider)),
            generation: Arc::new(AtomicU64::new(0)),
            confirmed_generation: Arc::new(AtomicU64::new(0)),
            publication: Arc::new(std::sync::Mutex::new(())),
        }
    }

    pub async fn replace(&self, provider: Arc<dyn Provider + Send + Sync>) {
        let mut current = self.current.write().await;
        let _publication = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *current = provider;
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
}

#[async_trait::async_trait]
impl Provider for ProviderSlot {
    async fn fetch(&self, repo: &RepoRef) -> Result<ProviderSnapshot, ProviderError> {
        let (provider, generation) = {
            let current = self.current.read().await;
            (current.clone(), self.generation.load(Ordering::Acquire))
        };
        let result = provider.fetch(repo).await;
        if self.generation.load(Ordering::Acquire) != generation {
            return Err(ProviderError::Superseded);
        }
        if result.is_ok() {
            self.confirmed_generation
                .store(generation, Ordering::Release);
        }
        result.map(|snapshot| snapshot.with_publication_generation(generation))
    }

    fn allows_cached_viewer_identity(&self) -> bool {
        self.confirmed_generation.load(Ordering::Acquire) == self.generation.load(Ordering::Acquire)
    }

    fn commit_if_current(&self, publication_generations: &[u64], commit: &mut dyn FnMut()) -> bool {
        let _publication = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let generation = self.generation.load(Ordering::Acquire);
        if publication_generations
            .iter()
            .any(|candidate| *candidate != generation)
        {
            return false;
        }
        commit();
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionAuth {
    Required,
    Disabled,
}

pub struct RuntimeOptions {
    pub address: String,
    pub session_auth: SessionAuth,
    pub issue: Option<NonZeroU64>,
    pub spaces_file: PathBuf,
    pub cache_root: PathBuf,
    pub poll_interval: Duration,
}

#[derive(Clone)]
pub struct RuntimeShutdown {
    sender: watch::Sender<bool>,
}

impl RuntimeShutdown {
    pub fn shutdown(&self) {
        self.sender.send_replace(true);
    }
}

pub struct ApplicationRuntime {
    address: SocketAddr,
    cockpit_url: String,
    state: Arc<AppState>,
    shutdown: RuntimeShutdown,
    server: Option<JoinHandle<io::Result<()>>>,
    poller: Option<JoinHandle<()>>,
    polling: PollingControl,
}

impl ApplicationRuntime {
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn cockpit_url(&self) -> &str {
        &self.cockpit_url
    }

    pub fn state(&self) -> Arc<AppState> {
        self.state.clone()
    }

    pub fn shutdown_handle(&self) -> RuntimeShutdown {
        self.shutdown.clone()
    }

    pub fn polling_control(&self) -> PollingControl {
        self.polling.clone()
    }

    pub async fn wait(mut self) -> Result<(), RuntimeError> {
        let server = self
            .server
            .take()
            .expect("application runtime server handle should be present");
        let server_result = server.await.map_err(RuntimeError::ServerTask)?;

        self.shutdown.shutdown();
        if let Some(poller) = self.poller.take() {
            poller.abort();
            match poller.await {
                Ok(()) => {}
                Err(error) if error.is_cancelled() => {}
                Err(error) => return Err(RuntimeError::PollerTask(error)),
            }
        }

        server_result.map_err(RuntimeError::Server)
    }
}

impl Drop for ApplicationRuntime {
    fn drop(&mut self) {
        self.shutdown.shutdown();
        if let Some(server) = &self.server {
            server.abort();
        }
        if let Some(poller) = &self.poller {
            poller.abort();
        }
    }
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("could not bind application server: {0}")]
    Bind(#[source] io::Error),
    #[error("could not read bound application address: {0}")]
    LocalAddress(#[source] io::Error),
    #[error("could not generate session token: {0}")]
    SessionToken(#[source] getrandom::Error),
    #[error("application server task failed: {0}")]
    ServerTask(#[source] tokio::task::JoinError),
    #[error("application server failed: {0}")]
    Server(#[source] io::Error),
    #[error("application poller task failed: {0}")]
    PollerTask(#[source] tokio::task::JoinError),
}

pub async fn start(
    options: RuntimeOptions,
    provider: Arc<dyn Provider + Send + Sync>,
) -> Result<ApplicationRuntime, RuntimeError> {
    let polling = PollingControl::fixed(options.poll_interval);
    start_with_polling(options, provider, polling).await
}

pub async fn start_with_polling(
    options: RuntimeOptions,
    provider: Arc<dyn Provider + Send + Sync>,
    polling: PollingControl,
) -> Result<ApplicationRuntime, RuntimeError> {
    let listener = tokio::net::TcpListener::bind(&options.address)
        .await
        .map_err(RuntimeError::Bind)?;
    let address = listener.local_addr().map_err(RuntimeError::LocalAddress)?;
    let session_token = match options.session_auth {
        SessionAuth::Required => Some(session_token().map_err(RuntimeError::SessionToken)?),
        SessionAuth::Disabled => None,
    };
    let spaces = SpaceStore::load(options.spaces_file);
    let (hub, _receiver) = tokio::sync::watch::channel(Model { spaces: vec![] });
    let state = Arc::new(AppState {
        hub,
        token: session_token.clone(),
        spaces: tokio::sync::Mutex::new(spaces),
        refresh: Arc::new(tokio::sync::Notify::new()),
    });
    let poller = spawn_controlled_poller(
        state.clone(),
        provider,
        Cache::new(options.cache_root),
        polling.subscribe(),
    );

    let (shutdown_sender, mut shutdown_receiver) = watch::channel(false);
    let server_state = state.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, router(server_state))
            .with_graceful_shutdown(async move {
                while !*shutdown_receiver.borrow_and_update() {
                    if shutdown_receiver.changed().await.is_err() {
                        break;
                    }
                }
            })
            .await
    });

    Ok(ApplicationRuntime {
        address,
        cockpit_url: cockpit_url(address, session_token.as_deref(), options.issue),
        state,
        shutdown: RuntimeShutdown {
            sender: shutdown_sender,
        },
        server: Some(server),
        poller: Some(poller),
        polling,
    })
}

fn cockpit_url(address: SocketAddr, token: Option<&str>, issue: Option<NonZeroU64>) -> String {
    let mut query = Vec::new();
    if let Some(token) = token {
        query.push(format!("token={token}"));
    }
    if let Some(issue) = issue {
        query.push(format!("issue={issue}"));
    }
    let suffix = if query.is_empty() {
        String::new()
    } else {
        format!("?{}", query.join("&"))
    };
    format!("http://{address}/{suffix}")
}

fn session_token() -> Result<String, getrandom::Error> {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)?;
    let mut token = String::with_capacity(32);
    for byte in bytes {
        token.push(HEX[(byte >> 4) as usize] as char);
        token.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::session_token;

    #[test]
    fn session_token_is_32_lowercase_hex_characters() {
        let token = session_token().unwrap();

        assert_eq!(token.len(), 32);
        assert!(
            token
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        );
    }
}
