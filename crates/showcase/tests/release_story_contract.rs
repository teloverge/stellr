use stellr_core::Status;
use stellr_showcase::{
    ClosureReason, IssueSnapshot, LifecycleEvent, LifecycleEventKind, MilestoneIdentity,
    PreviousRelease, RecordedIssue, ReleaseBoundaries, ReleaseEvidence, ReleaseStory,
    SnapshotState, StartingSnapshot, UtcTimestamp,
};

fn ts(value: &str) -> UtcTimestamp {
    value.parse().expect("valid UTC fixture timestamp")
}

fn snapshot(state: SnapshotState, assignees: &[&str]) -> IssueSnapshot {
    IssueSnapshot {
        state,
        assignees: assignees.iter().map(|value| (*value).to_owned()).collect(),
    }
}

fn issue(
    number: u64,
    milestone_id: Option<&str>,
    blocked_by: &[u64],
    starting_snapshot: StartingSnapshot,
    final_snapshot: IssueSnapshot,
) -> RecordedIssue {
    RecordedIssue {
        number,
        title: format!("Issue {number}"),
        url: format!("https://github.com/teloverge/stellr/issues/{number}"),
        milestone_id: milestone_id.map(str::to_owned),
        blocked_by: blocked_by.to_vec(),
        starting_snapshot,
        final_snapshot,
    }
}

fn event(
    provider_event_id: &str,
    occurred_at: &str,
    issue_number: u64,
    kind: LifecycleEventKind,
) -> LifecycleEvent {
    LifecycleEvent {
        provider_event_id: provider_event_id.to_owned(),
        occurred_at: ts(occurred_at),
        issue_number,
        kind,
    }
}

fn status_of(statuses: &[stellr_showcase::IssueStatus], issue_number: u64) -> Option<Status> {
    statuses
        .iter()
        .find(|status| status.issue_number == issue_number)
        .unwrap()
        .status
}

fn recorded_release() -> (ReleaseEvidence, ReleaseBoundaries) {
    let issues = vec![
        issue(
            5,
            None,
            &[],
            StartingSnapshot::Existing(snapshot(SnapshotState::Open, &[])),
            snapshot(SnapshotState::Closed, &[]),
        ),
        issue(
            10,
            None,
            &[5],
            StartingSnapshot::Existing(snapshot(SnapshotState::Open, &[])),
            snapshot(SnapshotState::Closed, &[]),
        ),
        issue(
            20,
            Some("M1"),
            &[10],
            StartingSnapshot::Existing(snapshot(SnapshotState::Open, &[])),
            snapshot(SnapshotState::Closed, &["teloverge"]),
        ),
        issue(
            30,
            Some("M1"),
            &[20],
            StartingSnapshot::Existing(snapshot(SnapshotState::Open, &[])),
            snapshot(SnapshotState::Open, &[]),
        ),
        issue(
            40,
            Some("M1"),
            &[],
            StartingSnapshot::NotCreated,
            snapshot(SnapshotState::ClosedNotPlanned, &[]),
        ),
    ];
    let events = vec![
        event(
            "E01",
            "2026-07-01T00:15:00Z",
            5,
            LifecycleEventKind::Closed {
                reason: ClosureReason::Completed,
            },
        ),
        event(
            "E02",
            "2026-07-01T00:30:00Z",
            10,
            LifecycleEventKind::Assigned {
                login: "teloverge".to_owned(),
            },
        ),
        event(
            "E03",
            "2026-07-01T00:45:00Z",
            10,
            LifecycleEventKind::Unassigned {
                login: "teloverge".to_owned(),
            },
        ),
        event(
            "E04",
            "2026-07-01T01:00:00Z",
            10,
            LifecycleEventKind::Closed {
                reason: ClosureReason::Completed,
            },
        ),
        event(
            "E05",
            "2026-07-01T01:15:00Z",
            20,
            LifecycleEventKind::Assigned {
                login: "teloverge".to_owned(),
            },
        ),
        event(
            "E06",
            "2026-07-01T01:30:00Z",
            20,
            LifecycleEventKind::Closed {
                reason: ClosureReason::Completed,
            },
        ),
        event(
            "E07",
            "2026-07-01T01:45:00Z",
            30,
            LifecycleEventKind::Closed {
                reason: ClosureReason::Completed,
            },
        ),
        event(
            "E08",
            "2026-07-01T02:00:00Z",
            30,
            LifecycleEventKind::Reopened,
        ),
        event(
            "E09",
            "2026-07-01T02:15:00Z",
            40,
            LifecycleEventKind::Opened,
        ),
        event(
            "E10",
            "2026-07-01T02:30:00Z",
            40,
            LifecycleEventKind::Closed {
                reason: ClosureReason::NotPlanned,
            },
        ),
    ];

    (
        ReleaseEvidence {
            repository: "teloverge/stellr".to_owned(),
            release_version: "v0.2.0".to_owned(),
            milestone: MilestoneIdentity {
                id: "M1".to_owned(),
                title: "v0.2.0".to_owned(),
            },
            issues,
            events,
        },
        ReleaseBoundaries {
            starting_cutoff: None,
            previous_release: Some(PreviousRelease {
                version: "v0.1.0".to_owned(),
                released_at: ts("2026-07-01T00:00:00Z"),
            }),
            ending_cutoff: Some(ts("2026-07-01T04:00:00Z")),
        },
    )
}

fn one_issue_release(
    final_snapshot: IssueSnapshot,
    events: Vec<LifecycleEvent>,
) -> (ReleaseEvidence, ReleaseBoundaries) {
    (
        ReleaseEvidence {
            repository: "teloverge/stellr".to_owned(),
            release_version: "v0.2.0".to_owned(),
            milestone: MilestoneIdentity {
                id: "M1".to_owned(),
                title: "v0.2.0".to_owned(),
            },
            issues: vec![issue(
                1,
                Some("M1"),
                &[],
                StartingSnapshot::Existing(snapshot(SnapshotState::Open, &[])),
                final_snapshot,
            )],
            events,
        },
        ReleaseBoundaries {
            starting_cutoff: Some(ts("2026-07-01T00:00:00Z")),
            previous_release: None,
            ending_cutoff: Some(ts("2026-07-01T01:00:00Z")),
        },
    )
}

#[test]
fn recorded_evidence_builds_a_deterministic_auditable_story() {
    let (evidence, boundaries) = recorded_release();
    let story = ReleaseStory::build(evidence.clone(), boundaries.clone()).unwrap();

    assert_eq!(story.visible_issue_numbers, vec![10, 20, 30, 40]);
    assert_eq!(story.hidden_support_issue_numbers, vec![5]);
    assert_eq!(
        story
            .final_topology
            .iter()
            .map(|edge| (edge.blocker, edge.dependent))
            .collect::<Vec<_>>(),
        vec![(10, 20), (20, 30)]
    );
    assert_eq!(story.evidence.issues.len(), 5);
    assert_eq!(story.evidence.events.len(), 10);
    assert_eq!(story.beats.len(), 8);
    assert_eq!(
        story
            .final_statuses
            .iter()
            .map(|status| (status.issue_number, status.status))
            .collect::<Vec<_>>(),
        vec![
            (10, Some(Status::Resolved)),
            (20, Some(Status::Resolved)),
            (30, Some(Status::Frontier)),
            (40, Some(Status::OutOfScope)),
        ]
    );
    assert_eq!(story.coordinates.len(), 4);
    assert_eq!(status_of(&story.initial_statuses, 40), None);
    assert!(
        story
            .beats
            .iter()
            .any(|beat| status_of(&beat.statuses, 20) == Some(Status::Claimed))
    );
    assert!(
        story
            .beats
            .iter()
            .any(|beat| status_of(&beat.statuses, 30) == Some(Status::Resolved))
    );
    assert!(
        story
            .beats
            .iter()
            .any(|beat| status_of(&beat.statuses, 40) == Some(Status::Frontier))
    );

    let mut reordered = evidence;
    reordered.issues.reverse();
    reordered.events.reverse();
    let reordered_story = ReleaseStory::build(reordered, boundaries).unwrap();

    assert_eq!(
        serde_json::to_vec(&story).unwrap(),
        serde_json::to_vec(&reordered_story).unwrap()
    );
}

#[test]
fn missing_provider_event_identity_fails_with_a_precise_diagnostic() {
    let (mut evidence, boundaries) = recorded_release();
    evidence.events[0].provider_event_id.clear();

    let error = ReleaseStory::build(evidence, boundaries).unwrap_err();

    assert_eq!(
        error.to_string(),
        "lifecycle event for issue #5 at 2026-07-01T00:15:00Z has no provider event ID"
    );
}

#[test]
fn event_without_recorded_issue_evidence_fails_closed() {
    let (mut evidence, boundaries) = recorded_release();
    evidence.events.push(event(
        "E99",
        "2026-07-01T03:00:00Z",
        999,
        LifecycleEventKind::Assigned {
            login: "teloverge".to_owned(),
        },
    ));

    let error = ReleaseStory::build(evidence, boundaries).unwrap_err();

    assert_eq!(
        error.to_string(),
        "event 'E99' references missing issue evidence for issue #999"
    );
}

#[test]
fn release_boundaries_are_explicit_and_unambiguous() {
    let (evidence, boundaries) = recorded_release();

    let mut missing_end = boundaries.clone();
    missing_end.ending_cutoff = None;
    assert_eq!(
        ReleaseStory::build(evidence.clone(), missing_end)
            .unwrap_err()
            .to_string(),
        "release story requires an explicit ending cutoff"
    );

    let mut missing_start = boundaries.clone();
    missing_start.previous_release = None;
    assert_eq!(
        ReleaseStory::build(evidence.clone(), missing_start)
            .unwrap_err()
            .to_string(),
        "first release requires a starting cutoff; later releases require a previous release"
    );

    let mut ambiguous = boundaries.clone();
    ambiguous.starting_cutoff = Some(ts("2026-07-01T00:00:00Z"));
    assert_eq!(
        ReleaseStory::build(evidence.clone(), ambiguous)
            .unwrap_err()
            .to_string(),
        "release boundary is ambiguous: provide either a starting cutoff or a previous release, not both"
    );

    let mut invalid_window = boundaries.clone();
    invalid_window.ending_cutoff = Some(ts("2026-07-01T00:00:00Z"));
    assert_eq!(
        ReleaseStory::build(evidence.clone(), invalid_window)
            .unwrap_err()
            .to_string(),
        "release ending cutoff must be later than its starting cutoff"
    );

    let mut missing_previous_identifier = boundaries.clone();
    missing_previous_identifier
        .previous_release
        .as_mut()
        .unwrap()
        .version
        .clear();
    assert_eq!(
        ReleaseStory::build(evidence.clone(), missing_previous_identifier)
            .unwrap_err()
            .to_string(),
        "later release requires a non-empty previous release identifier"
    );

    let mut first_release = boundaries;
    first_release.starting_cutoff = Some(ts("2026-07-01T00:00:00Z"));
    first_release.previous_release = None;
    assert!(ReleaseStory::build(evidence, first_release).is_ok());
}

#[test]
fn incomplete_or_ambiguous_lifecycle_evidence_fails_closed() {
    let (evidence, boundaries) = recorded_release();

    let mut missing_blocker = evidence.clone();
    missing_blocker.issues.retain(|issue| issue.number != 5);
    missing_blocker
        .events
        .retain(|event| event.issue_number != 5);
    assert_eq!(
        ReleaseStory::build(missing_blocker, boundaries.clone())
            .unwrap_err()
            .to_string(),
        "issue #10 references missing blocker evidence for issue #5"
    );

    let mut missing_event = evidence.clone();
    missing_event.events.pop();
    assert_eq!(
        ReleaseStory::build(missing_event, boundaries.clone())
            .unwrap_err()
            .to_string(),
        "missing lifecycle evidence for issue #40: reconstructed final state does not match the recorded cutoff state"
    );

    let mut ambiguous = evidence.clone();
    ambiguous.events[0].kind = LifecycleEventKind::Reopened;
    assert_eq!(
        ReleaseStory::build(ambiguous, boundaries.clone())
            .unwrap_err()
            .to_string(),
        "ambiguous lifecycle state at event 'E01' for issue #5: reopened event follows an open state"
    );

    let mut duplicate_assignment = evidence.clone();
    duplicate_assignment.events.push(event(
        "E05B",
        "2026-07-01T01:20:00Z",
        20,
        LifecycleEventKind::Assigned {
            login: "teloverge".to_owned(),
        },
    ));
    assert_eq!(
        ReleaseStory::build(duplicate_assignment, boundaries.clone())
            .unwrap_err()
            .to_string(),
        "ambiguous lifecycle state at event 'E05B' for issue #20: assigned login was already assigned"
    );

    let mut empty = evidence;
    empty.milestone.id = "not-recorded".to_owned();
    assert_eq!(
        ReleaseStory::build(empty, boundaries)
            .unwrap_err()
            .to_string(),
        "release milestone 'not-recorded' contains no issues"
    );
}

#[test]
fn a_release_without_a_visible_status_change_is_rejected() {
    let (evidence, boundaries) = one_issue_release(snapshot(SnapshotState::Open, &[]), Vec::new());

    assert_eq!(
        ReleaseStory::build(evidence, boundaries)
            .unwrap_err()
            .to_string(),
        "release story has no visible status change between its cutoffs"
    );
}

#[test]
fn candidates_less_than_ten_minutes_apart_share_one_beat() {
    let (evidence, boundaries) = one_issue_release(
        snapshot(SnapshotState::Open, &[]),
        vec![
            event(
                "E1",
                "2026-07-01T00:05:00Z",
                1,
                LifecycleEventKind::Assigned {
                    login: "teloverge".to_owned(),
                },
            ),
            event(
                "E2",
                "2026-07-01T00:14:59Z",
                1,
                LifecycleEventKind::Unassigned {
                    login: "teloverge".to_owned(),
                },
            ),
        ],
    );

    let story = ReleaseStory::build(evidence, boundaries).unwrap();

    assert_eq!(story.beats.len(), 1);
    assert_eq!(story.beats[0].source_event_ids, vec!["E1", "E2"]);
    assert!(
        story
            .evidence
            .events
            .iter()
            .all(|event| event.beat_index == Some(0))
    );
}

#[test]
fn non_status_events_remain_evidence_without_creating_a_beat() {
    let (evidence, boundaries) = one_issue_release(
        snapshot(SnapshotState::Open, &["alice", "bob"]),
        vec![
            event(
                "E1",
                "2026-07-01T00:05:00Z",
                1,
                LifecycleEventKind::Assigned {
                    login: "alice".to_owned(),
                },
            ),
            event(
                "E2",
                "2026-07-01T00:10:00Z",
                1,
                LifecycleEventKind::Assigned {
                    login: "bob".to_owned(),
                },
            ),
        ],
    );

    let story = ReleaseStory::build(evidence, boundaries).unwrap();

    assert_eq!(story.beats.len(), 1);
    assert_eq!(story.evidence.events[0].beat_index, Some(0));
    assert_eq!(story.evidence.events[1].beat_index, None);
}

#[test]
fn utc_timestamp_rejects_non_utc_offsets() {
    assert!("2026-07-01T00:00:00-05:00".parse::<UtcTimestamp>().is_err());
}
