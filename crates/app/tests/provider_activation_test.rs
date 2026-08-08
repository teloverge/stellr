use std::sync::Arc;

use stellr_app::runtime::ProviderSlot;
use stellr_core::{Provider, ProviderError, ProviderSnapshot, RepoRef};

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
            issues: vec![],
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
    fetch.await.unwrap().unwrap();

    assert!(!slot.allows_cached_viewer_identity());
}
