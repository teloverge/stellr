//! stellr-core: pure issue-graph domain.

pub mod derive;
pub mod history;
pub mod model;
pub mod provider;

pub use derive::derive;
pub use history::{
    HistoryEvent, HistoryEventKind, HistoryImportState, HistoryPage, HistoryPageRequest,
    HistorySummary, IssueSyncMetadata, MilestoneRef,
};
pub use model::{IssueState, Model, RawIssue, SpaceModel, Star, Status};
pub use provider::{Provider, ProviderError, ProviderSnapshot, RepoRef};
