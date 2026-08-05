use std::{
    future::pending,
    num::NonZeroU64,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use stellr_app::runtime::{RuntimeOptions, SessionAuth, start};
use stellr_core::{
    HistoryPage, HistoryPageRequest, IssueState, IssueSyncMetadata, Provider, ProviderError,
    ProviderSnapshot, RawIssue, RepoRef,
};
use stellr_server::spaces::{SpaceEntry, SpaceStore};
use tempfile::TempDir;
use tokio::{sync::Notify, time::timeout};

struct PendingProvider {
    fetch_started: Notify,
    fetch_dropped: Arc<AtomicBool>,
}

struct FetchDropSignal(Arc<AtomicBool>);

struct CompleteHistoryProvider {
    history_requests: AtomicUsize,
}

struct OfflineProvider;

impl Drop for FetchDropSignal {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[async_trait::async_trait]
impl Provider for PendingProvider {
    async fn fetch(&self, _repo: &RepoRef) -> Result<Vec<RawIssue>, ProviderError> {
        let _drop_signal = FetchDropSignal(self.fetch_dropped.clone());
        self.fetch_started.notify_one();
        pending().await
    }
}

fn historical_issue() -> RawIssue {
    RawIssue {
        number: 7,
        parent_issue: None,
        title: "Persisted issue".into(),
        body: String::new(),
        state: IssueState::Open,
        assignees: vec![],
        milestone: None,
        labels: vec![],
        blocked_by: vec![],
        url: "https://github.com/teloverge/stellr/issues/7".into(),
    }
}

#[async_trait::async_trait]
impl Provider for CompleteHistoryProvider {
    async fn fetch(&self, _repo: &RepoRef) -> Result<Vec<RawIssue>, ProviderError> {
        Ok(vec![historical_issue()])
    }

    async fn fetch_snapshot(&self, _repo: &RepoRef) -> Result<ProviderSnapshot, ProviderError> {
        Ok(ProviderSnapshot {
            repository_id: Some("R_stellr".into()),
            history_cutoff: Some(1_753_000_000),
            issues: vec![historical_issue()],
            history: vec![IssueSyncMetadata {
                issue_id: "I_7".into(),
                number: 7,
                created_at: 1_752_000_000,
                updated_at: 1_752_000_000,
                milestone: None,
            }],
        })
    }

    async fn fetch_history_page(
        &self,
        _repo: &RepoRef,
        _request: &HistoryPageRequest,
    ) -> Result<HistoryPage, ProviderError> {
        self.history_requests.fetch_add(1, Ordering::SeqCst);
        Ok(HistoryPage {
            events: vec![],
            next_cursor: None,
            resume_cursor: Some("CUR_END".into()),
            complete: true,
        })
    }
}

#[async_trait::async_trait]
impl Provider for OfflineProvider {
    async fn fetch(&self, _repo: &RepoRef) -> Result<Vec<RawIssue>, ProviderError> {
        Err(ProviderError::Http("offline".into()))
    }

    async fn fetch_history_page(
        &self,
        _repo: &RepoRef,
        _request: &HistoryPageRequest,
    ) -> Result<HistoryPage, ProviderError> {
        panic!("an offline restart must not refetch a completed timeline")
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

#[tokio::test]
async fn complete_history_survives_a_native_runtime_restart_while_offline() {
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

    let provider = Arc::new(CompleteHistoryProvider {
        history_requests: AtomicUsize::new(0),
    });
    let runtime = start(options(&profile), provider.clone()).await.unwrap();
    timeout(Duration::from_secs(5), async {
        loop {
            if runtime
                .state()
                .history
                .summary("teloverge-stellr")
                .unwrap()
                .is_some_and(|summary| summary.state == stellr_core::HistoryImportState::Complete)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the first run should persist a complete history ledger");
    assert_eq!(provider.history_requests.load(Ordering::SeqCst), 1);
    assert_eq!(
        runtime
            .state()
            .history
            .events_after("teloverge-stellr", 0)
            .unwrap()
            .len(),
        1
    );
    runtime.shutdown_handle().shutdown();
    runtime.wait().await.unwrap();

    let restarted = start(options(&profile), Arc::new(OfflineProvider))
        .await
        .unwrap();
    timeout(Duration::from_secs(5), async {
        loop {
            let model = restarted.state().hub.borrow().clone();
            if model
                .spaces
                .first()
                .is_some_and(|space| space.stale && space.error.is_some())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the offline restart should publish its cached current snapshot");
    let summary = restarted
        .state()
        .history
        .summary("teloverge-stellr")
        .unwrap()
        .unwrap();
    assert_eq!(summary.state, stellr_core::HistoryImportState::Complete);
    assert_eq!(
        restarted
            .state()
            .history
            .events_after("teloverge-stellr", 0)
            .unwrap()
            .len(),
        1
    );
    restarted.shutdown_handle().shutdown();
    restarted.wait().await.unwrap();
}
