use crate::{HistoryPage, HistoryPageRequest, IssueSyncMetadata, RawIssue};

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderSnapshot {
    pub viewer_login: Option<String>,
    pub repository_id: Option<String>,
    /// Provider-backed boundary through which history can be claimed complete.
    pub history_cutoff: Option<i64>,
    pub issues: Vec<RawIssue>,
    pub history: Vec<IssueSyncMetadata>,
    publication_generation: Option<u64>,
}

impl ProviderSnapshot {
    pub fn new(viewer_login: Option<String>, issues: Vec<RawIssue>) -> Self {
        Self {
            viewer_login,
            repository_id: None,
            history_cutoff: None,
            issues,
            history: Vec::new(),
            publication_generation: None,
        }
    }

    pub fn with_history(
        viewer_login: Option<String>,
        repository_id: Option<String>,
        history_cutoff: Option<i64>,
        issues: Vec<RawIssue>,
        history: Vec<IssueSyncMetadata>,
    ) -> Self {
        Self {
            viewer_login,
            repository_id,
            history_cutoff,
            issues,
            history,
            publication_generation: None,
        }
    }

    pub fn without_viewer(issues: Vec<RawIssue>) -> Self {
        Self::new(None, issues)
    }

    pub fn with_publication_generation(mut self, generation: u64) -> Self {
        self.publication_generation = Some(generation);
        self
    }

    pub fn publication_generation(&self) -> Option<u64> {
        self.publication_generation
    }
}

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
    async fn fetch(&self, repo: &RepoRef) -> Result<ProviderSnapshot, ProviderError>;

    async fn fetch_snapshot(&self, repo: &RepoRef) -> Result<ProviderSnapshot, ProviderError> {
        self.fetch(repo).await
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

    fn allows_cached_viewer_identity(&self) -> bool {
        true
    }

    fn commit_if_current(
        &self,
        _publication_generations: &[u64],
        commit: &mut dyn FnMut(),
    ) -> bool {
        commit();
        true
    }
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
    #[error("provider changed while the request was in flight")]
    Superseded,
    #[error("unsupported provider operation: {0}")]
    Unsupported(String),
}
