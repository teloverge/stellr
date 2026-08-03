use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use stellr_core::{Model, Provider, ProviderError, RawIssue, RepoRef};
use stellr_github::cache::Cache;
use stellr_server::{
    poll::{PollingControl, spawn_controlled_poller},
    spaces::{SpaceEntry, SpaceStore},
    state::AppState,
};

struct CountingProvider(Arc<AtomicUsize>);

#[async_trait::async_trait]
impl Provider for CountingProvider {
    async fn fetch(&self, _repo: &RepoRef) -> Result<Vec<RawIssue>, ProviderError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(vec![])
    }
}

async fn settle(calls: &AtomicUsize, expected: usize) {
    for _ in 0..100 {
        if calls.load(Ordering::SeqCst) == expected {
            return;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(calls.load(Ordering::SeqCst), expected);
}

#[tokio::test(start_paused = true)]
async fn focus_transitions_reschedule_one_poller_and_manual_refresh_stays_immediate() {
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
    let (hub, _) = tokio::sync::watch::channel(Model { spaces: vec![] });
    let state = Arc::new(AppState {
        hub,
        token: None,
        spaces: tokio::sync::Mutex::new(spaces),
        refresh: Arc::new(tokio::sync::Notify::new()),
    });
    let calls = Arc::new(AtomicUsize::new(0));
    let control = PollingControl::focus_aware(
        std::time::Duration::from_secs(30),
        std::time::Duration::from_secs(300),
    );
    let poller = spawn_controlled_poller(
        state.clone(),
        Arc::new(CountingProvider(calls.clone())),
        Cache::new(profile.path().join("cache")),
        control.subscribe(),
    );

    settle(&calls, 1).await;

    control.set_focused(false);
    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_secs(299)).await;
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    settle(&calls, 2).await;

    control.set_focused(true);
    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_secs(29)).await;
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    settle(&calls, 3).await;

    control.set_focused(false);
    tokio::task::yield_now().await;
    state.refresh.notify_one();
    settle(&calls, 4).await;

    poller.abort();
}
