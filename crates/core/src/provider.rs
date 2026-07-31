use crate::RawIssue;

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
}
