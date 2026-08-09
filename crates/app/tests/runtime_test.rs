use std::{
    future::pending,
    num::NonZeroU64,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use stellr_app::runtime::{RuntimeOptions, SessionAuth, start};
use stellr_core::{Provider, ProviderError, ProviderSnapshot, RepoRef};
use stellr_server::spaces::{SpaceEntry, SpaceStore};
use tempfile::TempDir;
use tokio::{sync::Notify, time::timeout};

struct PendingProvider {
    fetch_started: Notify,
    fetch_dropped: Arc<AtomicBool>,
}

struct FetchDropSignal(Arc<AtomicBool>);

impl Drop for FetchDropSignal {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[async_trait::async_trait]
impl Provider for PendingProvider {
    async fn fetch(&self, _repo: &RepoRef) -> Result<ProviderSnapshot, ProviderError> {
        let _drop_signal = FetchDropSignal(self.fetch_dropped.clone());
        self.fetch_started.notify_one();
        pending().await
    }
}

fn options(profile: &TempDir) -> RuntimeOptions {
    RuntimeOptions {
        address: "127.0.0.1:0".into(),
        session_auth: SessionAuth::Required,
        issue: NonZeroU64::new(58),
        spaces_file: profile.path().join("spaces.toml"),
        cache_root: profile.path().join("cache"),
        poll_interval: Duration::from_secs(30),
    }
}

#[tokio::test]
async fn runtime_exposes_its_loopback_contract_and_stops_server_and_poller() {
    let profile = tempfile::tempdir().unwrap();
    let mut spaces = SpaceStore::load(profile.path().join("spaces.toml"));
    spaces
        .add(SpaceEntry::new(
            RepoRef {
                owner: "teloverge".into(),
                name: "stellr".into(),
            },
            None,
        ))
        .unwrap();
    spaces.save().unwrap();

    let fetch_dropped = Arc::new(AtomicBool::new(false));
    let provider = Arc::new(PendingProvider {
        fetch_started: Notify::new(),
        fetch_dropped: fetch_dropped.clone(),
    });

    let runtime = start(options(&profile), provider.clone()).await.unwrap();
    assert!(runtime.address().ip().is_loopback());
    assert_ne!(runtime.address().port(), 0);
    assert_eq!(runtime.state().spaces.lock().await.entries().len(), 1);

    let cockpit_url = reqwest::Url::parse(runtime.cockpit_url()).unwrap();
    assert_eq!(cockpit_url.host_str(), Some("127.0.0.1"));
    assert_eq!(
        cockpit_url
            .query_pairs()
            .find_map(|(name, value)| (name == "issue").then(|| value.into_owned())),
        Some("58".into())
    );
    let session_token = cockpit_url
        .query_pairs()
        .find_map(|(name, value)| (name == "token").then(|| value.into_owned()))
        .expect("required session authentication should add a token");
    assert_eq!(session_token.len(), 32);
    assert_eq!(
        reqwest::get(cockpit_url.clone()).await.unwrap().status(),
        reqwest::StatusCode::OK
    );

    timeout(Duration::from_secs(2), provider.fetch_started.notified())
        .await
        .expect("the poller should begin synchronizing the stored space");

    let address = runtime.address();
    runtime.shutdown_handle().shutdown();
    timeout(Duration::from_secs(2), runtime.wait())
        .await
        .expect("graceful shutdown should be bounded")
        .unwrap();

    assert!(
        fetch_dropped.load(Ordering::SeqCst),
        "shutdown should cancel the in-flight provider fetch"
    );
    assert!(
        reqwest::get(format!("http://{address}/api/model"))
            .await
            .is_err(),
        "shutdown should stop accepting HTTP connections"
    );
}
