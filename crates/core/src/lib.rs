//! stellr-core: pure issue-graph domain.

pub mod model;
pub mod provider;

pub use model::{IssueState, Model, RawIssue, SpaceModel, Star, Status};
pub use provider::{Provider, ProviderError, RepoRef};
