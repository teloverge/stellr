use crate::story::{ReplayIssueState, apply_lifecycle_event};
use crate::{
    ClosureReason, IssueSnapshot, LifecycleEvent, LifecycleEventKind, MilestoneIdentity,
    PreviousRelease, RecordedIssue, ReleaseBoundaries, ReleaseEvidence, ReleaseStory,
    StartingSnapshot, StoryBuildError, UtcTimestamp,
};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use stellr_core::{ProviderError, RepoRef};
use stellr_github::auth;
use stellr_github::sync::GithubGraphqlClient;
use thiserror::Error;

const MILESTONES_QUERY: &str = r#"
query FetchShowcaseMilestones($owner: String!, $name: String!, $cursor: String) {
  repository(owner: $owner, name: $name) {
    milestones(first: 100, after: $cursor, states: [OPEN, CLOSED]) {
      pageInfo { hasNextPage endCursor }
      totalCount
      nodes { id title updatedAt }
    }
  }
}
"#;

const RELEASES_QUERY: &str = r#"
query FetchShowcaseReleases($owner: String!, $name: String!, $cursor: String) {
  repository(owner: $owner, name: $name) {
    releases(first: 100, after: $cursor) {
      pageInfo { hasNextPage endCursor }
      totalCount
      nodes { tagName publishedAt isDraft }
    }
  }
}
"#;

const ISSUES_QUERY: &str = r#"
query FetchShowcaseIssues($owner: String!, $name: String!, $cursor: String) {
  repository(owner: $owner, name: $name) {
    issues(first: 100, after: $cursor, states: [OPEN, CLOSED]) {
      pageInfo { hasNextPage endCursor }
      totalCount
      nodes {
        id
        number
        title
        url
        createdAt
        milestone { id }
      }
    }
  }
}
"#;

const BLOCKERS_QUERY: &str = r#"
query FetchShowcaseBlockers(
  $owner: String!, $name: String!, $number: Int!, $cursor: String
) {
  repository(owner: $owner, name: $name) {
    issue(number: $number) {
      blockedBy(first: 100, after: $cursor) {
        pageInfo { hasNextPage endCursor }
        totalCount
        nodes { ... on Issue { number } }
      }
    }
  }
}
"#;

const TIMELINE_QUERY: &str = r#"
query FetchShowcaseTimeline(
  $owner: String!, $name: String!, $number: Int!, $cursor: String
) {
  repository(owner: $owner, name: $name) {
    issue(number: $number) {
      timelineItems(
        first: 100,
        after: $cursor,
        itemTypes: [
          ASSIGNED_EVENT,
          BLOCKED_BY_ADDED_EVENT,
          BLOCKED_BY_REMOVED_EVENT,
          BLOCKING_ADDED_EVENT,
          BLOCKING_REMOVED_EVENT,
          CLOSED_EVENT,
          DEMILESTONED_EVENT,
          MILESTONED_EVENT,
          RENAMED_TITLE_EVENT,
          REOPENED_EVENT,
          UNASSIGNED_EVENT
        ]
      ) {
        pageInfo { hasNextPage endCursor }
        totalCount
        nodes {
          __typename
          ... on AssignedEvent {
            id
            createdAt
            assignee {
              ... on Bot { login }
              ... on Mannequin { login }
              ... on Organization { login }
              ... on User { login }
            }
          }
          ... on ClosedEvent { id createdAt stateReason }
          ... on BlockedByAddedEvent { id createdAt }
          ... on BlockedByRemovedEvent { id createdAt }
          ... on BlockingAddedEvent { id createdAt }
          ... on BlockingRemovedEvent { id createdAt }
          ... on DemilestonedEvent { id createdAt }
          ... on MilestonedEvent { id createdAt }
          ... on RenamedTitleEvent { id createdAt }
          ... on ReopenedEvent { id createdAt }
          ... on UnassignedEvent {
            id
            createdAt
            assignee {
              ... on Bot { login }
              ... on Mannequin { login }
              ... on Organization { login }
              ... on User { login }
            }
          }
        }
      }
    }
  }
}
"#;

/// User-selected live release identity and explicit historical boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveReleaseRequest {
    pub release_version: String,
    pub milestone_title: String,
    pub start: ReleaseWindowStart,
    pub ending_cutoff: UtcTimestamp,
}

/// The caller must explicitly declare whether this is the first or a later release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseWindowStart {
    FirstRelease { starting_cutoff: UtcTimestamp },
    PreviousRelease { tag: String },
}

/// Read-only source of a normalized release story.
#[async_trait]
pub trait ReleaseHistorySource {
    async fn build_story(
        &self,
        repository: &RepoRef,
        request: LiveReleaseRequest,
    ) -> Result<ReleaseStory, ReleaseHistoryError>;
}

/// GitHub-backed release history using the same transport policy as Stellr's runtime provider.
pub struct GithubReleaseHistorySource {
    client: GithubGraphqlClient,
}

impl GithubReleaseHistorySource {
    pub fn new() -> Result<Self, ReleaseHistoryError> {
        let token = auth::resolve_token()
            .map_err(|error| ReleaseHistoryError::Authentication(error.to_string()))?;
        Ok(Self {
            client: GithubGraphqlClient::new(token)?,
        })
    }

    pub fn with_token(token: String) -> Result<Self, ReleaseHistoryError> {
        Ok(Self {
            client: GithubGraphqlClient::new(token)?,
        })
    }

    pub fn with_base_uri(token: String, base: &str) -> Result<Self, ReleaseHistoryError> {
        Ok(Self {
            client: GithubGraphqlClient::with_base_uri(token, base)?,
        })
    }

    async fn fetch_connection<T, F>(
        &self,
        query: &'static str,
        stage: &'static str,
        field_path: &[&str],
        mut variables: F,
    ) -> Result<Vec<T>, ReleaseHistoryError>
    where
        T: DeserializeOwned,
        F: FnMut(Option<&str>) -> Value,
    {
        let mut cursor: Option<String> = None;
        let mut seen_cursors = BTreeSet::new();
        let mut expected_count = None;
        let mut nodes = Vec::new();

        loop {
            let value = self
                .client
                .post_value(&GraphqlRequest {
                    query,
                    variables: variables(cursor.as_deref()),
                })
                .await?;
            let connection = parse_connection::<T>(value, stage, field_path)?;
            let page_count = connection
                .total_count
                .ok_or_else(|| partial(stage, "connection did not report a total count"))?;
            if let Some(expected_count) = expected_count {
                if expected_count != page_count {
                    return Err(partial(stage, "connection total changed during pagination"));
                }
            } else {
                expected_count = Some(page_count);
            }

            for node in connection.nodes {
                nodes.push(node.ok_or_else(|| partial(stage, "connection returned a null node"))?);
            }

            if !connection.page_info.has_next_page {
                if nodes.len() != page_count {
                    return Err(partial(
                        stage,
                        format!(
                            "pagination returned {} of {page_count} reported nodes",
                            nodes.len()
                        ),
                    ));
                }
                return Ok(nodes);
            }

            let next = connection
                .page_info
                .end_cursor
                .ok_or_else(|| partial(stage, "next page has no end cursor"))?;
            if !seen_cursors.insert(next.clone()) {
                return Err(partial(
                    stage,
                    format!("pagination repeated cursor '{next}'"),
                ));
            }
            cursor = Some(next);
        }
    }

    async fn fetch_milestones(
        &self,
        repository: &RepoRef,
    ) -> Result<Vec<MilestoneNode>, ReleaseHistoryError> {
        self.fetch_connection(MILESTONES_QUERY, "milestones", &["milestones"], |cursor| {
            repository_variables(repository, cursor)
        })
        .await
    }

    async fn fetch_releases(
        &self,
        repository: &RepoRef,
    ) -> Result<Vec<ReleaseNode>, ReleaseHistoryError> {
        self.fetch_connection(RELEASES_QUERY, "releases", &["releases"], |cursor| {
            repository_variables(repository, cursor)
        })
        .await
    }

    async fn fetch_issues(
        &self,
        repository: &RepoRef,
    ) -> Result<Vec<IssueNode>, ReleaseHistoryError> {
        self.fetch_connection(ISSUES_QUERY, "issues", &["issues"], |cursor| {
            repository_variables(repository, cursor)
        })
        .await
    }

    async fn fetch_blockers(
        &self,
        repository: &RepoRef,
        issue_number: u64,
    ) -> Result<Vec<BlockerNode>, ReleaseHistoryError> {
        self.fetch_connection(
            BLOCKERS_QUERY,
            "issue blockers",
            &["issue", "blockedBy"],
            |cursor| issue_variables(repository, issue_number, cursor),
        )
        .await
    }

    async fn fetch_timeline(
        &self,
        repository: &RepoRef,
        issue_number: u64,
    ) -> Result<Vec<TimelineNode>, ReleaseHistoryError> {
        self.fetch_connection(
            TIMELINE_QUERY,
            "issue timeline",
            &["issue", "timelineItems"],
            |cursor| issue_variables(repository, issue_number, cursor),
        )
        .await
    }

    async fn resolve_boundaries(
        &self,
        repository: &RepoRef,
        request: &LiveReleaseRequest,
    ) -> Result<ReleaseBoundaries, ReleaseHistoryError> {
        match &request.start {
            ReleaseWindowStart::FirstRelease { starting_cutoff } => Ok(ReleaseBoundaries {
                starting_cutoff: Some(*starting_cutoff),
                previous_release: None,
                ending_cutoff: Some(request.ending_cutoff),
            }),
            ReleaseWindowStart::PreviousRelease { tag } if tag.trim().is_empty() => {
                Err(StoryBuildError::MissingPreviousReleaseIdentifier.into())
            }
            ReleaseWindowStart::PreviousRelease { tag } => {
                let releases = self.fetch_releases(repository).await?;
                let mut matching = releases
                    .into_iter()
                    .filter(|release| !release.is_draft && release.tag_name == *tag)
                    .collect::<Vec<_>>();
                if matching.is_empty() {
                    return Err(ReleaseHistoryError::PreviousReleaseNotFound { tag: tag.clone() });
                }
                if matching.len() > 1 {
                    return Err(ReleaseHistoryError::AmbiguousPreviousRelease { tag: tag.clone() });
                }
                let release = matching.pop().expect("checked one matching release");
                let published_at = release.published_at.ok_or_else(|| {
                    partial(
                        "releases",
                        format!("release '{tag}' has no publication timestamp"),
                    )
                })?;
                Ok(ReleaseBoundaries {
                    starting_cutoff: None,
                    previous_release: Some(PreviousRelease {
                        version: tag.clone(),
                        released_at: parse_timestamp("release publication", &published_at)?,
                    }),
                    ending_cutoff: Some(request.ending_cutoff),
                })
            }
        }
    }
}

#[async_trait]
impl ReleaseHistorySource for GithubReleaseHistorySource {
    async fn build_story(
        &self,
        repository: &RepoRef,
        request: LiveReleaseRequest,
    ) -> Result<ReleaseStory, ReleaseHistoryError> {
        let boundaries = self.resolve_boundaries(repository, &request).await?;
        let starting_cutoff = boundaries
            .starting_cutoff
            .or_else(|| {
                boundaries
                    .previous_release
                    .as_ref()
                    .map(|release| release.released_at)
            })
            .expect("validated start boundary");
        let ending_cutoff = boundaries.ending_cutoff.expect("validated ending cutoff");

        let milestones = self.fetch_milestones(repository).await?;
        let mut matching_milestones = milestones
            .into_iter()
            .filter(|milestone| milestone.title == request.milestone_title)
            .collect::<Vec<_>>();
        if matching_milestones.is_empty() {
            return Err(ReleaseHistoryError::MilestoneNotFound {
                title: request.milestone_title,
            });
        }
        if matching_milestones.len() > 1 {
            return Err(ReleaseHistoryError::AmbiguousMilestone {
                title: request.milestone_title,
            });
        }
        let milestone = matching_milestones.pop().expect("checked one milestone");
        let milestone_updated_at = parse_timestamp("milestone update", &milestone.updated_at)?;
        if milestone_updated_at > ending_cutoff {
            return Err(ReleaseHistoryError::SnapshotNewerThanCutoff {
                cutoff: ending_cutoff,
                detail: format!(
                    "milestone '{}' changed at {milestone_updated_at}",
                    milestone.title
                ),
            });
        }

        let mut issue_nodes = self.fetch_issues(repository).await?;
        for issue in &mut issue_nodes {
            issue.parsed_created_at = Some(parse_timestamp("issue creation", &issue.created_at)?);
        }
        issue_nodes.retain(|issue| {
            issue
                .parsed_created_at
                .is_some_and(|time| time <= ending_cutoff)
        });
        issue_nodes.sort_by_key(|issue| issue.number);

        let mut recorded_issues = Vec::with_capacity(issue_nodes.len());
        let mut window_events = Vec::new();
        for issue in issue_nodes {
            let mut blockers = self
                .fetch_blockers(repository, issue.number)
                .await?
                .into_iter()
                .map(|blocker| blocker.number)
                .collect::<Vec<_>>();
            blockers.sort_unstable();
            blockers.dedup();

            let created_at = issue.parsed_created_at.expect("parsed before retain");
            let mut timeline = Vec::new();
            for node in self.fetch_timeline(repository, issue.number).await? {
                match timeline_record(issue.number, node)? {
                    TimelineRecord::Lifecycle(event) => timeline.push(event),
                    TimelineRecord::SnapshotMutation(mutation) => {
                        if mutation.occurred_at < created_at {
                            return Err(partial(
                                "issue timeline",
                                format!("issue #{} has an event before its creation", issue.number),
                            ));
                        }
                        if mutation.occurred_at > ending_cutoff {
                            return Err(ReleaseHistoryError::SnapshotNewerThanCutoff {
                                cutoff: ending_cutoff,
                                detail: format!(
                                    "issue #{} {} changed at {} (event {})",
                                    issue.number,
                                    mutation.kind.description(),
                                    mutation.occurred_at,
                                    mutation.provider_event_id
                                ),
                            });
                        }
                    }
                }
            }
            timeline.sort_by(|left, right| {
                left.occurred_at
                    .cmp(&right.occurred_at)
                    .then_with(|| left.provider_event_id.cmp(&right.provider_event_id))
            });
            if timeline.iter().any(|event| event.occurred_at < created_at) {
                return Err(partial(
                    "issue timeline",
                    format!("issue #{} has an event before its creation", issue.number),
                ));
            }

            let starting_snapshot =
                match snapshot_at(issue.number, created_at, &timeline, starting_cutoff)? {
                    Some(snapshot) => StartingSnapshot::Existing(snapshot),
                    None => StartingSnapshot::NotCreated,
                };
            let final_snapshot = snapshot_at(issue.number, created_at, &timeline, ending_cutoff)?
                .ok_or_else(|| {
                partial(
                    "issues",
                    format!("issue #{} did not exist at cutoff", issue.number),
                )
            })?;

            if created_at > starting_cutoff {
                window_events.push(LifecycleEvent {
                    provider_event_id: format!("issue:{}:opened", issue.id),
                    occurred_at: created_at,
                    issue_number: issue.number,
                    kind: LifecycleEventKind::Opened,
                });
            }
            window_events.extend(timeline.into_iter().filter(|event| {
                event.occurred_at > starting_cutoff && event.occurred_at <= ending_cutoff
            }));
            recorded_issues.push(RecordedIssue {
                number: issue.number,
                title: issue.title,
                url: issue.url,
                milestone_id: issue.milestone.map(|milestone| milestone.id),
                blocked_by: blockers,
                starting_snapshot,
                final_snapshot,
            });
        }

        Ok(ReleaseStory::build(
            ReleaseEvidence {
                repository: repository.slug(),
                release_version: request.release_version,
                milestone: MilestoneIdentity {
                    id: milestone.id,
                    title: milestone.title,
                },
                issues: recorded_issues,
                events: window_events,
            },
            boundaries,
        )?)
    }
}

/// A live release history could not be acquired or normalized completely.
#[derive(Debug, Error)]
pub enum ReleaseHistoryError {
    #[error("GitHub authentication could not be resolved: {0}")]
    Authentication(String),
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    Story(#[from] StoryBuildError),
    #[error("milestone '{title}' was not found")]
    MilestoneNotFound { title: String },
    #[error("milestone title '{title}' matched more than one milestone")]
    AmbiguousMilestone { title: String },
    #[error("previous release '{tag}' was not found")]
    PreviousReleaseNotFound { tag: String },
    #[error("previous release identifier '{tag}' matched more than one release")]
    AmbiguousPreviousRelease { tag: String },
    #[error("current GitHub snapshot is newer than cutoff {cutoff}: {detail}")]
    SnapshotNewerThanCutoff {
        cutoff: UtcTimestamp,
        detail: String,
    },
    #[error("partial GitHub {stage}: {detail}")]
    Partial { stage: &'static str, detail: String },
}

#[derive(Serialize)]
struct GraphqlRequest {
    query: &'static str,
    variables: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlConnection<T> {
    page_info: PageInfo,
    #[serde(default)]
    total_count: Option<usize>,
    nodes: Vec<Option<T>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Deserialize)]
struct MilestoneNode {
    id: String,
    title: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReleaseNode {
    tag_name: String,
    published_at: Option<String>,
    is_draft: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IssueNode {
    id: String,
    number: u64,
    title: String,
    url: String,
    created_at: String,
    milestone: Option<IssueMilestone>,
    #[serde(skip)]
    parsed_created_at: Option<UtcTimestamp>,
}

#[derive(Deserialize)]
struct IssueMilestone {
    id: String,
}

#[derive(Deserialize)]
struct BlockerNode {
    number: u64,
}

#[derive(Deserialize)]
#[serde(tag = "__typename")]
enum TimelineNode {
    #[serde(rename = "AssignedEvent")]
    Assigned {
        id: String,
        #[serde(rename = "createdAt")]
        created_at: String,
        assignee: Option<AssigneeNode>,
    },
    #[serde(rename = "ClosedEvent")]
    Closed {
        id: String,
        #[serde(rename = "createdAt")]
        created_at: String,
        #[serde(rename = "stateReason")]
        state_reason: Option<String>,
    },
    #[serde(rename = "BlockedByAddedEvent")]
    BlockedByAdded {
        id: String,
        #[serde(rename = "createdAt")]
        created_at: String,
    },
    #[serde(rename = "BlockedByRemovedEvent")]
    BlockedByRemoved {
        id: String,
        #[serde(rename = "createdAt")]
        created_at: String,
    },
    #[serde(rename = "BlockingAddedEvent")]
    BlockingAdded {
        id: String,
        #[serde(rename = "createdAt")]
        created_at: String,
    },
    #[serde(rename = "BlockingRemovedEvent")]
    BlockingRemoved {
        id: String,
        #[serde(rename = "createdAt")]
        created_at: String,
    },
    #[serde(rename = "DemilestonedEvent")]
    Demilestoned {
        id: String,
        #[serde(rename = "createdAt")]
        created_at: String,
    },
    #[serde(rename = "MilestonedEvent")]
    Milestoned {
        id: String,
        #[serde(rename = "createdAt")]
        created_at: String,
    },
    #[serde(rename = "RenamedTitleEvent")]
    RenamedTitle {
        id: String,
        #[serde(rename = "createdAt")]
        created_at: String,
    },
    #[serde(rename = "ReopenedEvent")]
    Reopened {
        id: String,
        #[serde(rename = "createdAt")]
        created_at: String,
    },
    #[serde(rename = "UnassignedEvent")]
    Unassigned {
        id: String,
        #[serde(rename = "createdAt")]
        created_at: String,
        assignee: Option<AssigneeNode>,
    },
}

#[derive(Deserialize)]
struct AssigneeNode {
    login: String,
}

fn repository_variables(repository: &RepoRef, cursor: Option<&str>) -> Value {
    json!({
        "owner": repository.owner,
        "name": repository.name,
        "cursor": cursor,
    })
}

fn issue_variables(repository: &RepoRef, issue_number: u64, cursor: Option<&str>) -> Value {
    json!({
        "owner": repository.owner,
        "name": repository.name,
        "number": issue_number,
        "cursor": cursor,
    })
}

fn parse_connection<T: DeserializeOwned>(
    value: Value,
    stage: &'static str,
    field_path: &[&str],
) -> Result<GraphqlConnection<T>, ReleaseHistoryError> {
    let mut current = value
        .get("data")
        .and_then(|data| data.get("repository"))
        .ok_or_else(|| ProviderError::Parse(format!("missing data.repository during {stage}")))?;
    if current.is_null() {
        return Err(ProviderError::Parse(format!("missing data.repository during {stage}")).into());
    }
    for field in field_path {
        current = current.get(*field).ok_or_else(|| {
            ProviderError::Parse(format!("missing {} during {stage}", field_path.join(".")))
        })?;
        if current.is_null() {
            return Err(partial(stage, format!("{} is null", field_path.join("."))));
        }
    }
    serde_json::from_value(current.clone())
        .map_err(|error| ProviderError::Parse(format!("{stage}: {error}")).into())
}

fn parse_timestamp(stage: &'static str, value: &str) -> Result<UtcTimestamp, ReleaseHistoryError> {
    value
        .parse()
        .map_err(|error| ProviderError::Parse(format!("{stage}: {error}")).into())
}

enum TimelineRecord {
    Lifecycle(LifecycleEvent),
    SnapshotMutation(SnapshotMutation),
}

struct SnapshotMutation {
    provider_event_id: String,
    occurred_at: UtcTimestamp,
    kind: SnapshotMutationKind,
}

enum SnapshotMutationKind {
    Milestone,
    BlockerTopology,
    Title,
}

impl SnapshotMutationKind {
    fn description(&self) -> &'static str {
        match self {
            Self::Milestone => "milestone",
            Self::BlockerTopology => "blocker topology",
            Self::Title => "title",
        }
    }
}

fn timeline_record(
    issue_number: u64,
    node: TimelineNode,
) -> Result<TimelineRecord, ReleaseHistoryError> {
    let mutation = match &node {
        TimelineNode::BlockedByAdded { id, created_at }
        | TimelineNode::BlockedByRemoved { id, created_at }
        | TimelineNode::BlockingAdded { id, created_at }
        | TimelineNode::BlockingRemoved { id, created_at } => Some((
            id.clone(),
            created_at.clone(),
            SnapshotMutationKind::BlockerTopology,
        )),
        TimelineNode::Demilestoned { id, created_at }
        | TimelineNode::Milestoned { id, created_at } => Some((
            id.clone(),
            created_at.clone(),
            SnapshotMutationKind::Milestone,
        )),
        TimelineNode::RenamedTitle { id, created_at } => {
            Some((id.clone(), created_at.clone(), SnapshotMutationKind::Title))
        }
        _ => None,
    };
    if let Some((provider_event_id, created_at, kind)) = mutation {
        if provider_event_id.trim().is_empty() {
            return Err(partial(
                "issue timeline",
                "timeline event has no provider ID",
            ));
        }
        return Ok(TimelineRecord::SnapshotMutation(SnapshotMutation {
            provider_event_id,
            occurred_at: parse_timestamp("timeline event", &created_at)?,
            kind,
        }));
    }

    let (provider_event_id, created_at, kind) = match node {
        TimelineNode::Assigned {
            id,
            created_at,
            assignee,
        } => (
            id,
            created_at,
            LifecycleEventKind::Assigned {
                login: assignee
                    .ok_or_else(|| partial("issue timeline", "assigned event has no assignee"))?
                    .login,
            },
        ),
        TimelineNode::Closed {
            id,
            created_at,
            state_reason,
        } => {
            let reason = match state_reason.as_deref() {
                Some("NOT_PLANNED") => ClosureReason::NotPlanned,
                None | Some("COMPLETED") => ClosureReason::Completed,
                Some(reason) => {
                    return Err(partial(
                        "issue timeline",
                        format!("closed event '{id}' has unknown state reason '{reason}'"),
                    ));
                }
            };
            (id, created_at, LifecycleEventKind::Closed { reason })
        }
        TimelineNode::Reopened { id, created_at } => (id, created_at, LifecycleEventKind::Reopened),
        TimelineNode::Unassigned {
            id,
            created_at,
            assignee,
        } => (
            id,
            created_at,
            LifecycleEventKind::Unassigned {
                login: assignee
                    .ok_or_else(|| partial("issue timeline", "unassigned event has no assignee"))?
                    .login,
            },
        ),
        TimelineNode::BlockedByAdded { .. }
        | TimelineNode::BlockedByRemoved { .. }
        | TimelineNode::BlockingAdded { .. }
        | TimelineNode::BlockingRemoved { .. }
        | TimelineNode::Demilestoned { .. }
        | TimelineNode::Milestoned { .. }
        | TimelineNode::RenamedTitle { .. } => unreachable!("handled as snapshot mutation"),
    };
    if provider_event_id.trim().is_empty() {
        return Err(partial(
            "issue timeline",
            "timeline event has no provider ID",
        ));
    }
    Ok(TimelineRecord::Lifecycle(LifecycleEvent {
        provider_event_id,
        occurred_at: parse_timestamp("timeline event", &created_at)?,
        issue_number,
        kind,
    }))
}

fn snapshot_at(
    issue_number: u64,
    created_at: UtcTimestamp,
    timeline: &[LifecycleEvent],
    cutoff: UtcTimestamp,
) -> Result<Option<IssueSnapshot>, ReleaseHistoryError> {
    if created_at > cutoff {
        return Ok(None);
    }
    let mut state = Some(ReplayIssueState::open());
    for event in timeline.iter().filter(|event| event.occurred_at <= cutoff) {
        apply_lifecycle_event(&mut state, &event.kind).map_err(|error| {
            partial(
                "issue timeline",
                format!(
                    "event '{}' for issue #{issue_number} is ambiguous: {}",
                    event.provider_event_id,
                    error.detail()
                ),
            )
        })?;
    }
    Ok(state.map(ReplayIssueState::into_snapshot))
}

fn partial(stage: &'static str, detail: impl Into<String>) -> ReleaseHistoryError {
    ReleaseHistoryError::Partial {
        stage,
        detail: detail.into(),
    }
}
