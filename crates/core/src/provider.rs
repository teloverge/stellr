use crate::{HistoryPage, HistoryPageRequest, IssueSyncMetadata, RawIssue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRef {
    pub owner: String,
    pub name: String,
}

impl RepoRef {
    pub fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

#[async_trait::async_trait]
pub trait Provider {
    async fn fetch(&self, repo: &RepoRef) -> Result<Vec<RawIssue>, ProviderError>;

    async fn fetch_snapshot(&self, repo: &RepoRef) -> Result<ProviderSnapshot, ProviderError> {
        Ok(ProviderSnapshot {
            repository_id: None,
            history_cutoff: None,
            issues: self.fetch(repo).await?,
            history: Vec::new(),
        })
    }

    async fn fetch_history_page(
        &self,
        _repo: &RepoRef,
        _request: &HistoryPageRequest,
    ) -> Result<HistoryPage, ProviderError> {
        Err(ProviderError::Unsupported(
            "provider does not supply temporal history".into(),
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderSnapshot {
    pub repository_id: Option<String>,
    /// Provider-backed boundary through which history can be claimed complete.
    pub history_cutoff: Option<i64>,
    pub issues: Vec<RawIssue>,
    pub history: Vec<IssueSyncMetadata>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("GitHub rate limit exceeded")]
    RateLimited { reset_epoch: Option<i64> },
    #[error("HTTP request failed: {0}")]
    Http(String),
    #[error("response parsing failed: {0}")]
    Parse(String),
    #[error("unsupported provider operation: {0}")]
    Unsupported(String),
}
