use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MilestoneRef {
    pub id: Option<String>,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HistoryEventKind {
    IssueCreated {
        milestone: Option<MilestoneRef>,
    },
    IssueClosed,
    IssueReopened,
    MilestoneChanged {
        from: Option<MilestoneRef>,
        to: Option<MilestoneRef>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEvent {
    pub sequence: u64,
    pub repository_id: String,
    pub issue_id: String,
    pub issue_number: u64,
    pub provider_event_id: String,
    pub occurred_at: i64,
    #[serde(flatten)]
    pub kind: HistoryEventKind,
}

impl HistoryEvent {
    pub fn creation_id(issue_id: &str) -> String {
        format!("{issue_id}:issue_created")
    }
}

impl Ord for HistoryEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.occurred_at, self.provider_event_id.as_str())
            .cmp(&(other.occurred_at, other.provider_event_id.as_str()))
    }
}

impl PartialOrd for HistoryEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryImportState {
    #[default]
    Unavailable,
    Building,
    Complete,
    Delayed,
    RateLimited,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistorySummary {
    pub state: HistoryImportState,
    pub completed_issues: u64,
    pub total_issues: u64,
    pub earliest_event_at: Option<i64>,
    pub verified_through: Option<i64>,
    pub revision: u64,
    pub diagnostic: Option<String>,
    pub resume_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueSyncMetadata {
    pub issue_id: String,
    pub number: u64,
    pub created_at: i64,
    pub updated_at: i64,
    pub milestone: Option<MilestoneRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryPageRequest {
    pub issue_id: String,
    pub issue_number: u64,
    pub cursor: Option<String>,
    pub cutoff: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryPage {
    pub events: Vec<HistoryEvent>,
    pub next_cursor: Option<String>,
    pub complete: bool,
}
