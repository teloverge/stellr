use std::sync::Arc;

use stellr_app::runtime::ProviderSlot;
use stellr_core::{Provider, ProviderError, RawIssue, RepoRef};

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

    assert_eq!(slot.fetch(&repo).await.unwrap(), vec![]);
}
