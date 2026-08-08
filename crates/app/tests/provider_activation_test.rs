use std::{sync::Arc, time::Duration};

use stellr_app::runtime::{ProviderSlot, RuntimeOptions, SessionAuth, start};
use stellr_core::{IssueState, Provider, ProviderError, ProviderSnapshot, RawIssue, RepoRef};
use stellr_github::cache::Cache;
use stellr_server::spaces::{SpaceEntry, SpaceStore};

struct SignedOut;

#[async_trait::async_trait]
impl Provider for SignedOut {
    async fn fetch(&self, _repo: &RepoRef) -> Result<ProviderSnapshot, ProviderError> {
        Err(ProviderError::Auth("sign-in required".into()))
    }
}

struct Active;

#[async_trait::async_trait]
impl Provider for Active {
    async fn fetch(&self, _repo: &RepoRef) -> Result<ProviderSnapshot, ProviderError> {
        Ok(ProviderSnapshot::without_viewer(vec![]))
    }
}

#[tokio::test]
async fn replacing_the_provider_activates_it_in_the_current_process() {
    let slot = ProviderSlot::new(Arc::new(SignedOut));
    let repo = RepoRef {
        owner: "teloverge".into(),
        name: "stellr".into(),
    };

    assert!(matches!(
        slot.fetch(&repo).await,
        Err(ProviderError::Auth(_))
    ));

    slot.replace(Arc::new(Active)).await;

    assert_eq!(slot.fetch(&repo).await.unwrap().issues, vec![]);
}

struct DelayedSuccess {
    started: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
}

#[async_trait::async_trait]
impl Provider for DelayedSuccess {
    async fn fetch(&self, _repo: &RepoRef) -> Result<ProviderSnapshot, ProviderError> {
        self.started.notify_one();
        self.release.notified().await;
        Ok(ProviderSnapshot {
            viewer_login: Some("previous-account".into()),
            issues: vec![RawIssue {
                number: 92,
                parent_issue: None,
                title: "Previous account work".into(),
                body: String::new(),
                state: IssueState::Open,
                assignees: vec!["previous-account".into()],
                milestone: None,
                labels: vec!["ready-for-agent".into()],
                blocked_by: vec![],
                url: "https://github.com/teloverge/stellr/issues/92".into(),
            }],
        })
    }
}

#[tokio::test]
async fn replacement_suppresses_cached_identity_until_the_new_provider_succeeds() {
    let slot = ProviderSlot::new(Arc::new(Active));
    let repo = RepoRef {
        owner: "teloverge".into(),
        name: "stellr".into(),
    };
    assert!(slot.allows_cached_viewer_identity());

    slot.replace(Arc::new(SignedOut)).await;
    assert!(!slot.allows_cached_viewer_identity());
    assert!(slot.fetch(&repo).await.is_err());
    assert!(!slot.allows_cached_viewer_identity());

    slot.replace(Arc::new(Active)).await;
    assert!(!slot.allows_cached_viewer_identity());
    slot.fetch(&repo).await.unwrap();
    assert!(slot.allows_cached_viewer_identity());
}

#[tokio::test]
async fn an_old_in_flight_success_cannot_confirm_a_replacement_generation() {
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let slot = ProviderSlot::new(Arc::new(DelayedSuccess {
        started: started.clone(),
        release: release.clone(),
    }));
    let repo = RepoRef {
        owner: "teloverge".into(),
        name: "stellr".into(),
    };
    let started_wait = started.notified();
    let fetching_slot = slot.clone();
    let fetching_repo = repo.clone();
    let fetch = tokio::spawn(async move { fetching_slot.fetch(&fetching_repo).await });
    started_wait.await;

    slot.replace(Arc::new(SignedOut)).await;
    release.notify_one();
    assert!(matches!(
        fetch.await.unwrap(),
        Err(ProviderError::Superseded)
    ));

    assert!(!slot.allows_cached_viewer_identity());
}

#[tokio::test]
async fn a_superseded_fetch_never_reaches_the_live_model_or_cache() {
    let profile = tempfile::tempdir().unwrap();
    let repo = RepoRef {
        owner: "teloverge".into(),
        name: "stellr".into(),
    };
    let spaces_file = profile.path().join("spaces.toml");
    let cache_root = profile.path().join("cache");
    let mut spaces = SpaceStore::load(spaces_file.clone());
    spaces.add(SpaceEntry::new(repo.clone(), None)).unwrap();
    spaces.save().unwrap();

    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let slot = Arc::new(ProviderSlot::new(Arc::new(DelayedSuccess {
        started: started.clone(),
        release: release.clone(),
    })));
    let started_wait = started.notified();
    let runtime = start(
        RuntimeOptions {
            address: "127.0.0.1:0".into(),
            session_auth: SessionAuth::Disabled,
            issue: None,
            spaces_file,
            cache_root: cache_root.clone(),
            poll_interval: Duration::from_secs(60),
        },
        slot.clone(),
    )
    .await
    .unwrap();
    let state = runtime.state();
    let mut models = state.hub.subscribe();
    started_wait.await;

    slot.replace(Arc::new(SignedOut)).await;
    release.notify_one();

    tokio::time::timeout(Duration::from_secs(2), models.changed())
        .await
        .expect("superseded fetch should publish safe fallback state")
        .unwrap();
    let model = models.borrow_and_update().clone();
    assert_eq!(model.spaces.len(), 1);
    assert_eq!(model.spaces[0].viewer_login, None);
    assert!(model.spaces[0].stars.is_empty());
    assert!(model.spaces[0].stale);
    assert!(
        model.spaces[0]
            .error
            .as_deref()
            .is_some_and(|error| error.contains("provider changed"))
    );
    assert!(Cache::new(cache_root).load(&repo).is_none());

    runtime.shutdown_handle().shutdown();
    runtime.wait().await.unwrap();
}
