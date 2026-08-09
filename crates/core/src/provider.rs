use crate::RawIssue;

#[derive(Debug, Clone)]
pub struct ProviderSnapshot {
    pub viewer_login: Option<String>,
    pub issues: Vec<RawIssue>,
    publication_generation: Option<u64>,
}

impl ProviderSnapshot {
    pub fn new(viewer_login: Option<String>, issues: Vec<RawIssue>) -> Self {
        Self {
            viewer_login,
            issues,
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
}
