use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use stellr_app::{auth_activation::activate_provider_and_store, runtime::ProviderSlot};
use stellr_core::{Provider, ProviderError, RawIssue, RepoRef};
use stellr_github::{
    credentials::{CredentialStore, CredentialStoreError},
    device_flow::AccessToken,
};
use tokio::sync::Notify;

struct SignedOut;

#[async_trait::async_trait]
impl Provider for SignedOut {
    async fn fetch(&self, _repo: &RepoRef) -> Result<Vec<RawIssue>, ProviderError> {
        Err(ProviderError::Auth("sign-in required".into()))
    }
}

struct Active;

#[async_trait::async_trait]
impl Provider for Active {
    async fn fetch(&self, _repo: &RepoRef) -> Result<Vec<RawIssue>, ProviderError> {
        Ok(vec![])
    }
}

struct FailingStore {
    attempted: Arc<AtomicBool>,
}

impl CredentialStore for FailingStore {
    fn load(&self) -> Result<Option<String>, CredentialStoreError> {
        Ok(None)
    }

    fn store(&self, _credential: &str) -> Result<(), CredentialStoreError> {
        self.attempted.store(true, Ordering::SeqCst);
        Err(CredentialStoreError::Message("vault unavailable".into()))
    }
}

#[tokio::test]
async fn storage_failure_warns_after_activating_the_provider_and_refreshing() {
    let slot = ProviderSlot::new(Arc::new(SignedOut));
    let refresh = Arc::new(Notify::new());
    let attempted = Arc::new(AtomicBool::new(false));
    let refresh_wait = refresh.notified();

    let warning = activate_provider_and_store(
        &slot,
        Arc::new(Active),
        refresh.clone(),
        Arc::new(FailingStore {
            attempted: attempted.clone(),
        }),
        AccessToken::from("current-run-token".to_owned()),
    )
    .await;

    assert!(attempted.load(Ordering::SeqCst));
    assert!(warning.unwrap().contains("vault unavailable"));
    refresh_wait.await;
    let repo = RepoRef {
        owner: "teloverge".into(),
        name: "stellr".into(),
    };
    assert_eq!(slot.fetch(&repo).await.unwrap(), vec![]);
}
