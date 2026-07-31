//! stellr-core: pure issue-graph domain.

pub mod derive;
pub mod model;
pub mod provider;

pub use derive::derive;
pub use model::{IssueState, Model, RawIssue, SpaceModel, Star, Status};
pub use provider::{Provider, ProviderError, RepoRef};
