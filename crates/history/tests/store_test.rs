use stellr_core::{HistoryEventKind, HistoryImportState, IssueSyncMetadata, MilestoneRef};
use stellr_history::{HistoryStore, RepositorySeed};

fn issue(
    issue_id: &str,
    number: u64,
    created_at: i64,
    updated_at: i64,
    milestone: Option<(&str, &str)>,
) -> IssueSyncMetadata {
    IssueSyncMetadata {
        issue_id: issue_id.into(),
        number,
        created_at,
        updated_at,
        milestone: milestone.map(|(id, title)| MilestoneRef {
            id: id.into(),
            title: title.into(),
        }),
    }
}

#[test]
fn initializes_creation_history_idempotently_and_reads_ordered_deltas() {
    let temp = tempfile::tempdir().unwrap();
    let store = HistoryStore::open(temp.path().join("history.sqlite3")).unwrap();
    let seed = RepositorySeed {
        space_id: "octocat-hello".into(),
        provider_repository_id: "R_repo".into(),
        verified_through: 500,
        issues: vec![
            issue("I_later", 2, 200, 450, None),
            issue("I_first", 1, 100, 400, Some(("M_v1", "v1"))),
        ],
    };

    let first_summary = store.initialize_repository(&seed).unwrap();
    let first_events = store.events_after("octocat-hello", 0).unwrap();
    let repeated_summary = store.initialize_repository(&seed).unwrap();
    let repeated_events = store.events_after("octocat-hello", 0).unwrap();

    assert_eq!(first_summary, repeated_summary);
    assert_eq!(first_events, repeated_events);
    assert_eq!(first_events.len(), 2);
    assert_eq!(first_events[0].issue_number, 1);
    assert_eq!(first_events[1].issue_number, 2);
    assert!(matches!(
        &first_events[0].kind,
        HistoryEventKind::IssueCreated {
            milestone: Some(milestone)
        } if milestone.id == "M_v1" && milestone.title == "v1"
    ));
    assert_eq!(first_summary.state, HistoryImportState::Complete);
    assert_eq!(first_summary.completed_issues, 2);
    assert_eq!(first_summary.total_issues, 2);
    assert_eq!(first_summary.earliest_event_at, Some(100));
    assert_eq!(first_summary.verified_through, Some(500));
    assert_eq!(first_summary.revision, first_events[1].sequence);

    assert_eq!(
        store
            .events_after("octocat-hello", first_events[0].sequence)
            .unwrap(),
        vec![first_events[1].clone()]
    );
}

#[test]
fn removal_is_explicit_and_scoped_to_one_repository() {
    let temp = tempfile::tempdir().unwrap();
    let store = HistoryStore::open(temp.path().join("history.sqlite3")).unwrap();
    for (space_id, repository_id, number) in [("one", "R_one", 1), ("two", "R_two", 2)] {
        store
            .initialize_repository(&RepositorySeed {
                space_id: space_id.into(),
                provider_repository_id: repository_id.into(),
                verified_through: 500,
                issues: vec![issue(&format!("I_{number}"), number, 100, 200, None)],
            })
            .unwrap();
    }

    assert!(store.remove_repository("one").unwrap());

    assert!(store.summary("one").unwrap().is_none());
    assert!(store.events_after("one", 0).unwrap().is_empty());
    assert_eq!(store.events_after("two", 0).unwrap().len(), 1);
    assert!(!store.remove_repository("missing").unwrap());
}
