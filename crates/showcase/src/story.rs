use chrono::{DateTime, FixedOffset, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::str::FromStr;
use stellr_core::{IssueState, RawIssue, Status, derive};
use thiserror::Error;

const SCHEMA_VERSION: u32 = 1;
const REPLAY_MILLISECONDS: u32 = 8_000;
const MAX_BEATS: usize = 8;
const GROUPING_SECONDS: i64 = 10 * 60;

/// A parsed RFC 3339 timestamp whose offset is explicitly UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UtcTimestamp(DateTime<Utc>);

impl UtcTimestamp {
    fn signed_seconds_since(self, earlier: Self) -> i64 {
        self.0.signed_duration_since(earlier.0).num_seconds()
    }
}

impl fmt::Display for UtcTimestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0.to_rfc3339_opts(SecondsFormat::AutoSi, true))
    }
}

impl FromStr for UtcTimestamp {
    type Err = UtcTimestampError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let parsed: DateTime<FixedOffset> = value
            .parse()
            .map_err(|_| UtcTimestampError(value.to_owned()))?;
        if parsed.offset().local_minus_utc() != 0 {
            return Err(UtcTimestampError(value.to_owned()));
        }
        Ok(Self(parsed.with_timezone(&Utc)))
    }
}

/// A timestamp did not name an explicit UTC instant.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("'{0}' is not an explicit RFC 3339 UTC timestamp")]
pub struct UtcTimestampError(String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MilestoneIdentity {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviousRelease {
    pub version: String,
    pub released_at: UtcTimestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseBoundaries {
    pub starting_cutoff: Option<UtcTimestamp>,
    pub previous_release: Option<PreviousRelease>,
    pub ending_cutoff: Option<UtcTimestamp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotState {
    Open,
    Closed,
    ClosedNotPlanned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueSnapshot {
    pub state: SnapshotState,
    pub assignees: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "presence", content = "snapshot")]
pub enum StartingSnapshot {
    NotCreated,
    Existing(IssueSnapshot),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedIssue {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub milestone_id: Option<String>,
    pub blocked_by: Vec<u64>,
    pub starting_snapshot: StartingSnapshot,
    pub final_snapshot: IssueSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClosureReason {
    Completed,
    NotPlanned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum LifecycleEventKind {
    Opened,
    Closed { reason: ClosureReason },
    Reopened,
    Assigned { login: String },
    Unassigned { login: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleEvent {
    pub provider_event_id: String,
    pub occurred_at: UtcTimestamp,
    pub issue_number: u64,
    pub kind: LifecycleEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseEvidence {
    pub repository: String,
    pub release_version: String,
    pub milestone: MilestoneIdentity,
    pub issues: Vec<RecordedIssue>,
    pub events: Vec<LifecycleEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryBoundaries {
    pub starting_cutoff: UtcTimestamp,
    pub previous_release: Option<PreviousRelease>,
    pub ending_cutoff: UtcTimestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryEvidence {
    pub issues: Vec<RecordedIssue>,
    pub events: Vec<NormalizedLifecycleEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedLifecycleEvent {
    pub provider_event_id: String,
    pub occurred_at: UtcTimestamp,
    pub issue_number: u64,
    pub kind: LifecycleEventKind,
    pub beat_index: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryEdge {
    pub blocker: u64,
    pub dependent: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueStatus {
    pub issue_number: u64,
    pub status: Option<Status>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryBeat {
    pub index: usize,
    pub replay_offset_milliseconds: u32,
    pub source_event_ids: Vec<String>,
    pub statuses: Vec<IssueStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeCoordinate {
    pub issue_number: u64,
    pub x: u32,
    pub y: u32,
}

/// The deterministic, reviewable input to the release-constellation renderers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseStory {
    pub schema_version: u32,
    pub generator_version: String,
    pub repository: String,
    pub release_version: String,
    pub milestone: MilestoneIdentity,
    pub boundaries: StoryBoundaries,
    pub evidence: StoryEvidence,
    pub visible_issue_numbers: Vec<u64>,
    pub hidden_support_issue_numbers: Vec<u64>,
    pub final_topology: Vec<StoryEdge>,
    pub initial_statuses: Vec<IssueStatus>,
    pub beats: Vec<StoryBeat>,
    pub final_statuses: Vec<IssueStatus>,
    pub coordinates: Vec<NodeCoordinate>,
}

/// A release story could not be reconstructed without guessing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StoryBuildError {
    #[error("release story requires an explicit ending cutoff")]
    MissingEndingCutoff,
    #[error("first release requires a starting cutoff; later releases require a previous release")]
    MissingStartingBoundary,
    #[error(
        "release boundary is ambiguous: provide either a starting cutoff or a previous release, not both"
    )]
    AmbiguousStartingBoundary,
    #[error("release ending cutoff must be later than its starting cutoff")]
    InvalidWindow,
    #[error("later release requires a non-empty previous release identifier")]
    MissingPreviousReleaseIdentifier,
    #[error("release milestone '{milestone_id}' contains no issues")]
    EmptyConstellation { milestone_id: String },
    #[error("issue #{issue_number} is recorded more than once")]
    DuplicateIssue { issue_number: u64 },
    #[error("provider event ID '{provider_event_id}' is recorded more than once")]
    DuplicateEvent { provider_event_id: String },
    #[error("lifecycle event for issue #{issue_number} at {occurred_at} has no provider event ID")]
    MissingEventIdentity {
        issue_number: u64,
        occurred_at: UtcTimestamp,
    },
    #[error("issue #{dependent} references missing blocker evidence for issue #{blocker}")]
    MissingBlockerEvidence { dependent: u64, blocker: u64 },
    #[error(
        "event '{provider_event_id}' references missing issue evidence for issue #{issue_number}"
    )]
    MissingEventIssueEvidence {
        provider_event_id: String,
        issue_number: u64,
    },
    #[error("event '{provider_event_id}' for issue #{issue_number} is outside the release window")]
    EventOutsideWindow {
        provider_event_id: String,
        issue_number: u64,
    },
    #[error(
        "ambiguous lifecycle state at event '{provider_event_id}' for issue #{issue_number}: {detail}"
    )]
    AmbiguousState {
        provider_event_id: String,
        issue_number: u64,
        detail: String,
    },
    #[error(
        "missing lifecycle evidence for issue #{issue_number}: reconstructed final state does not match the recorded cutoff state"
    )]
    MissingLifecycleEvidence { issue_number: u64 },
    #[error("release story has no visible status change between its cutoffs")]
    NoVisibleStatusChange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReplayIssueState {
    state: SnapshotState,
    assignees: BTreeSet<String>,
}

impl ReplayIssueState {
    pub(crate) fn open() -> Self {
        Self {
            state: SnapshotState::Open,
            assignees: BTreeSet::new(),
        }
    }

    pub(crate) fn from_snapshot(snapshot: &IssueSnapshot) -> Self {
        Self {
            state: snapshot.state,
            assignees: snapshot.assignees.iter().cloned().collect(),
        }
    }

    pub(crate) fn into_snapshot(self) -> IssueSnapshot {
        IssueSnapshot {
            state: self.state,
            assignees: self.assignees.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LifecycleTransitionError(&'static str);

impl LifecycleTransitionError {
    pub(crate) fn detail(self) -> &'static str {
        self.0
    }
}

pub(crate) fn apply_lifecycle_event(
    state: &mut Option<ReplayIssueState>,
    kind: &LifecycleEventKind,
) -> Result<(), LifecycleTransitionError> {
    match kind {
        LifecycleEventKind::Opened => {
            if state.is_some() {
                return Err(LifecycleTransitionError(
                    "opened event follows an existing issue state",
                ));
            }
            *state = Some(ReplayIssueState::open());
        }
        LifecycleEventKind::Closed { reason } => {
            let state = state.as_mut().ok_or(LifecycleTransitionError(
                "closed event has no prior issue state",
            ))?;
            if state.state != SnapshotState::Open {
                return Err(LifecycleTransitionError(
                    "closed event follows a non-open state",
                ));
            }
            state.state = match reason {
                ClosureReason::Completed => SnapshotState::Closed,
                ClosureReason::NotPlanned => SnapshotState::ClosedNotPlanned,
            };
        }
        LifecycleEventKind::Reopened => {
            let state = state.as_mut().ok_or(LifecycleTransitionError(
                "reopened event has no prior issue state",
            ))?;
            if state.state == SnapshotState::Open {
                return Err(LifecycleTransitionError(
                    "reopened event follows an open state",
                ));
            }
            state.state = SnapshotState::Open;
        }
        LifecycleEventKind::Assigned { login } => {
            if login.trim().is_empty() {
                return Err(LifecycleTransitionError(
                    "assigned event has an empty login",
                ));
            }
            let state = state.as_mut().ok_or(LifecycleTransitionError(
                "assigned event has no prior issue state",
            ))?;
            if !state.assignees.insert(login.clone()) {
                return Err(LifecycleTransitionError(
                    "assigned login was already assigned",
                ));
            }
        }
        LifecycleEventKind::Unassigned { login } => {
            let state = state.as_mut().ok_or(LifecycleTransitionError(
                "unassigned event has no prior issue state",
            ))?;
            if !state.assignees.remove(login) {
                return Err(LifecycleTransitionError(
                    "unassigned login was not assigned",
                ));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct CandidateBeat {
    event_index: usize,
    occurred_at: UtcTimestamp,
    provider_event_id: String,
    statuses: Vec<IssueStatus>,
}

#[derive(Debug, Clone)]
struct CandidateGroup {
    candidates: Vec<CandidateBeat>,
}

impl ReleaseStory {
    pub fn build(
        mut evidence: ReleaseEvidence,
        boundaries: ReleaseBoundaries,
    ) -> Result<Self, StoryBuildError> {
        let boundaries = validate_boundaries(boundaries)?;
        normalize_and_sort_issues(&mut evidence.issues)?;
        normalize_and_sort_events(&mut evidence.events)?;

        let issues_by_number: BTreeMap<u64, &RecordedIssue> = evidence
            .issues
            .iter()
            .map(|issue| (issue.number, issue))
            .collect();
        for event in &evidence.events {
            if !issues_by_number.contains_key(&event.issue_number) {
                return Err(StoryBuildError::MissingEventIssueEvidence {
                    provider_event_id: event.provider_event_id.clone(),
                    issue_number: event.issue_number,
                });
            }
        }
        let (visible, support) = scope_issues(&issues_by_number, &evidence.milestone.id)?;

        let visible_issue_numbers = visible.iter().copied().collect::<Vec<_>>();
        let hidden_support_issue_numbers = support.difference(&visible).copied().collect();
        let scoped_issues = support
            .iter()
            .map(|number| (*issues_by_number[number]).clone())
            .collect::<Vec<_>>();
        let final_topology = build_visible_topology(&scoped_issues, &visible);

        let mut states = starting_states(&scoped_issues);
        let initial_statuses = derive_visible_statuses(&scoped_issues, &states, &visible);
        let mut prior_statuses = initial_statuses.clone();
        let mut normalized_events = Vec::new();
        let mut candidates = Vec::new();

        for event in evidence
            .events
            .iter()
            .filter(|event| support.contains(&event.issue_number))
        {
            if event.occurred_at <= boundaries.starting_cutoff
                || event.occurred_at > boundaries.ending_cutoff
            {
                return Err(StoryBuildError::EventOutsideWindow {
                    provider_event_id: event.provider_event_id.clone(),
                    issue_number: event.issue_number,
                });
            }

            apply_event(&mut states, event)?;
            let statuses = derive_visible_statuses(&scoped_issues, &states, &visible);
            let event_index = normalized_events.len();
            normalized_events.push(NormalizedLifecycleEvent {
                provider_event_id: event.provider_event_id.clone(),
                occurred_at: event.occurred_at,
                issue_number: event.issue_number,
                kind: event.kind.clone(),
                beat_index: None,
            });
            if statuses != prior_statuses {
                candidates.push(CandidateBeat {
                    event_index,
                    occurred_at: event.occurred_at,
                    provider_event_id: event.provider_event_id.clone(),
                    statuses: statuses.clone(),
                });
                prior_statuses = statuses;
            }
        }

        verify_final_states(&scoped_issues, &states)?;
        if candidates.is_empty() {
            return Err(StoryBuildError::NoVisibleStatusChange);
        }

        let groups = group_candidates(candidates);
        let beat_count = groups.len();
        let mut beats = Vec::with_capacity(beat_count);
        for (index, group) in groups.into_iter().enumerate() {
            let source_event_ids = group
                .candidates
                .iter()
                .map(|candidate| candidate.provider_event_id.clone())
                .collect();
            for candidate in &group.candidates {
                normalized_events[candidate.event_index].beat_index = Some(index);
            }
            beats.push(StoryBeat {
                index,
                replay_offset_milliseconds: ((index + 1) as u32 * REPLAY_MILLISECONDS)
                    / beat_count as u32,
                source_event_ids,
                statuses: group
                    .candidates
                    .last()
                    .expect("nonempty group")
                    .statuses
                    .clone(),
            });
        }

        let final_statuses = derive_visible_statuses(&scoped_issues, &states, &visible);
        let coordinates = deterministic_coordinates(&visible_issue_numbers);

        Ok(Self {
            schema_version: SCHEMA_VERSION,
            generator_version: env!("CARGO_PKG_VERSION").to_owned(),
            repository: evidence.repository,
            release_version: evidence.release_version,
            milestone: evidence.milestone,
            boundaries,
            evidence: StoryEvidence {
                issues: scoped_issues,
                events: normalized_events,
            },
            visible_issue_numbers,
            hidden_support_issue_numbers,
            final_topology,
            initial_statuses,
            beats,
            final_statuses,
            coordinates,
        })
    }
}

fn validate_boundaries(boundaries: ReleaseBoundaries) -> Result<StoryBoundaries, StoryBuildError> {
    let ending_cutoff = boundaries
        .ending_cutoff
        .ok_or(StoryBuildError::MissingEndingCutoff)?;
    let (starting_cutoff, previous_release) =
        match (boundaries.starting_cutoff, boundaries.previous_release) {
            (Some(starting_cutoff), None) => (starting_cutoff, None),
            (None, Some(previous_release)) => {
                if previous_release.version.trim().is_empty() {
                    return Err(StoryBuildError::MissingPreviousReleaseIdentifier);
                }
                (previous_release.released_at, Some(previous_release))
            }
            (None, None) => return Err(StoryBuildError::MissingStartingBoundary),
            (Some(_), Some(_)) => return Err(StoryBuildError::AmbiguousStartingBoundary),
        };
    if ending_cutoff <= starting_cutoff {
        return Err(StoryBuildError::InvalidWindow);
    }
    Ok(StoryBoundaries {
        starting_cutoff,
        previous_release,
        ending_cutoff,
    })
}

fn normalize_and_sort_issues(issues: &mut [RecordedIssue]) -> Result<(), StoryBuildError> {
    issues.sort_by_key(|issue| issue.number);
    for pair in issues.windows(2) {
        if pair[0].number == pair[1].number {
            return Err(StoryBuildError::DuplicateIssue {
                issue_number: pair[0].number,
            });
        }
    }
    for issue in issues {
        issue.blocked_by.sort_unstable();
        issue.blocked_by.dedup();
        normalize_starting_snapshot(&mut issue.starting_snapshot);
        normalize_snapshot(&mut issue.final_snapshot);
    }
    Ok(())
}

fn normalize_starting_snapshot(snapshot: &mut StartingSnapshot) {
    if let StartingSnapshot::Existing(snapshot) = snapshot {
        normalize_snapshot(snapshot);
    }
}

fn normalize_snapshot(snapshot: &mut IssueSnapshot) {
    snapshot.assignees.sort();
    snapshot.assignees.dedup();
}

fn normalize_and_sort_events(events: &mut [LifecycleEvent]) -> Result<(), StoryBuildError> {
    let mut ids = BTreeSet::new();
    for event in events.iter() {
        if event.provider_event_id.trim().is_empty() {
            return Err(StoryBuildError::MissingEventIdentity {
                issue_number: event.issue_number,
                occurred_at: event.occurred_at,
            });
        }
        if !ids.insert(event.provider_event_id.clone()) {
            return Err(StoryBuildError::DuplicateEvent {
                provider_event_id: event.provider_event_id.clone(),
            });
        }
    }
    events.sort_by(|left, right| {
        left.occurred_at
            .cmp(&right.occurred_at)
            .then_with(|| left.provider_event_id.cmp(&right.provider_event_id))
    });
    Ok(())
}

fn scope_issues(
    issues: &BTreeMap<u64, &RecordedIssue>,
    milestone_id: &str,
) -> Result<(BTreeSet<u64>, BTreeSet<u64>), StoryBuildError> {
    let milestone_issues = issues
        .values()
        .filter(|issue| issue.milestone_id.as_deref() == Some(milestone_id))
        .map(|issue| issue.number)
        .collect::<Vec<_>>();
    if milestone_issues.is_empty() {
        return Err(StoryBuildError::EmptyConstellation {
            milestone_id: milestone_id.to_owned(),
        });
    }

    let mut visible = milestone_issues.iter().copied().collect::<BTreeSet<_>>();
    for number in milestone_issues {
        for blocker in &issues[&number].blocked_by {
            require_blocker(issues, number, *blocker)?;
            visible.insert(*blocker);
        }
    }

    let mut support = visible.clone();
    let mut queue = visible.iter().copied().collect::<VecDeque<_>>();
    while let Some(dependent) = queue.pop_front() {
        for blocker in &issues[&dependent].blocked_by {
            require_blocker(issues, dependent, *blocker)?;
            if support.insert(*blocker) {
                queue.push_back(*blocker);
            }
        }
    }
    Ok((visible, support))
}

fn require_blocker(
    issues: &BTreeMap<u64, &RecordedIssue>,
    dependent: u64,
    blocker: u64,
) -> Result<(), StoryBuildError> {
    if !issues.contains_key(&blocker) {
        return Err(StoryBuildError::MissingBlockerEvidence { dependent, blocker });
    }
    Ok(())
}

fn build_visible_topology(issues: &[RecordedIssue], visible: &BTreeSet<u64>) -> Vec<StoryEdge> {
    let mut edges = issues
        .iter()
        .filter(|issue| visible.contains(&issue.number))
        .flat_map(|issue| {
            issue
                .blocked_by
                .iter()
                .filter(|blocker| visible.contains(blocker))
                .map(|blocker| StoryEdge {
                    blocker: *blocker,
                    dependent: issue.number,
                })
        })
        .collect::<Vec<_>>();
    edges.sort_by_key(|edge| (edge.blocker, edge.dependent));
    edges
}

fn starting_states(issues: &[RecordedIssue]) -> BTreeMap<u64, ReplayIssueState> {
    issues
        .iter()
        .filter_map(|issue| match &issue.starting_snapshot {
            StartingSnapshot::NotCreated => None,
            StartingSnapshot::Existing(snapshot) => {
                Some((issue.number, ReplayIssueState::from_snapshot(snapshot)))
            }
        })
        .collect()
}

fn apply_event(
    states: &mut BTreeMap<u64, ReplayIssueState>,
    event: &LifecycleEvent,
) -> Result<(), StoryBuildError> {
    let mut state = states.remove(&event.issue_number);
    if let Err(error) = apply_lifecycle_event(&mut state, &event.kind) {
        return Err(StoryBuildError::AmbiguousState {
            provider_event_id: event.provider_event_id.clone(),
            issue_number: event.issue_number,
            detail: error.detail().to_owned(),
        });
    }
    if let Some(state) = state {
        states.insert(event.issue_number, state);
    }
    Ok(())
}

fn derive_visible_statuses(
    issues: &[RecordedIssue],
    states: &BTreeMap<u64, ReplayIssueState>,
    visible: &BTreeSet<u64>,
) -> Vec<IssueStatus> {
    let raw_issues = issues
        .iter()
        .filter_map(|issue| {
            states.get(&issue.number).map(|state| RawIssue {
                number: issue.number,
                parent_issue: None,
                title: issue.title.clone(),
                body: String::new(),
                state: match state.state {
                    SnapshotState::Open => IssueState::Open,
                    SnapshotState::Closed => IssueState::Closed,
                    SnapshotState::ClosedNotPlanned => IssueState::ClosedNotPlanned,
                },
                assignees: state.assignees.iter().cloned().collect(),
                milestone: issue.milestone_id.clone(),
                labels: Vec::new(),
                blocked_by: issue.blocked_by.clone(),
                url: issue.url.clone(),
            })
        })
        .collect::<Vec<_>>();
    let derived = derive(&raw_issues)
        .into_iter()
        .map(|star| (star.number, star.status))
        .collect::<BTreeMap<_, _>>();
    visible
        .iter()
        .map(|number| IssueStatus {
            issue_number: *number,
            status: derived.get(number).copied(),
        })
        .collect()
}

fn verify_final_states(
    issues: &[RecordedIssue],
    states: &BTreeMap<u64, ReplayIssueState>,
) -> Result<(), StoryBuildError> {
    for issue in issues {
        let expected = ReplayIssueState::from_snapshot(&issue.final_snapshot);
        if states.get(&issue.number) != Some(&expected) {
            return Err(StoryBuildError::MissingLifecycleEvidence {
                issue_number: issue.number,
            });
        }
    }
    Ok(())
}

fn group_candidates(candidates: Vec<CandidateBeat>) -> Vec<CandidateGroup> {
    let mut groups: Vec<CandidateGroup> = Vec::new();
    for candidate in candidates {
        let merge_with_previous = groups.last().is_some_and(|group| {
            candidate
                .occurred_at
                .signed_seconds_since(group.candidates.last().expect("nonempty group").occurred_at)
                < GROUPING_SECONDS
        });
        if merge_with_previous {
            groups
                .last_mut()
                .expect("checked previous group")
                .candidates
                .push(candidate);
        } else {
            groups.push(CandidateGroup {
                candidates: vec![candidate],
            });
        }
    }

    while groups.len() > MAX_BEATS {
        let merge_index = (0..groups.len() - 1)
            .min_by_key(|index| {
                let left = groups[*index].candidates.last().expect("nonempty group");
                let right = groups[*index + 1]
                    .candidates
                    .first()
                    .expect("nonempty group");
                (
                    right.occurred_at.signed_seconds_since(left.occurred_at),
                    left.occurred_at,
                    left.provider_event_id.clone(),
                )
            })
            .expect("more than one group");
        let right = groups.remove(merge_index + 1);
        groups[merge_index].candidates.extend(right.candidates);
    }
    groups
}

fn deterministic_coordinates(issue_numbers: &[u64]) -> Vec<NodeCoordinate> {
    let columns = issue_numbers.len().min(4) as u32;
    let rows = (issue_numbers.len() as u32).div_ceil(columns);
    issue_numbers
        .iter()
        .enumerate()
        .map(|(index, issue_number)| {
            let index = index as u32;
            NodeCoordinate {
                issue_number: *issue_number,
                x: ((index % columns) + 1) * 1_200 / (columns + 1),
                y: ((index / columns) + 1) * 675 / (rows + 1),
            }
        })
        .collect()
}
