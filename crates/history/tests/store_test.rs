use stellr_core::{
    HistoryEvent, HistoryEventKind, HistoryImportState, IssueSyncMetadata, MilestoneRef,
};
use stellr_history::{HistoryStore, PageCheckpoint, RepositorySeed};

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
            id: Some(id.into()),
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
        timeline_required: false,
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
        } if milestone.id.as_deref() == Some("M_v1") && milestone.title == "v1"
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
                timeline_required: false,
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

#[test]
fn checkpoints_lifecycle_pages_atomically_and_resumes_without_duplicates() {
    let temp = tempfile::tempdir().unwrap();
    let store = HistoryStore::open(temp.path().join("history.sqlite3")).unwrap();
    let seed = RepositorySeed {
        space_id: "octocat-hello".into(),
        provider_repository_id: "R_repo".into(),
        verified_through: 500,
        timeline_required: true,
        issues: vec![issue("I_1", 1, 100, 400, None)],
    };

    let building = store.initialize_repository(&seed).unwrap();
    let pending = store.pending_issue("octocat-hello").unwrap().unwrap();

    assert_eq!(building.state, HistoryImportState::Building);
    assert_eq!(building.completed_issues, 0);
    assert_eq!(building.total_issues, 1);
    assert_eq!(building.verified_through, None);
    assert_eq!(pending.issue_id, "I_1");
    assert_eq!(pending.cursor, None);
    assert_eq!(pending.cutoff, 500);

    let event = |provider_event_id: &str, occurred_at, kind| HistoryEvent {
        sequence: 0,
        repository_id: "R_repo".into(),
        issue_id: "I_1".into(),
        issue_number: 1,
        provider_event_id: provider_event_id.into(),
        occurred_at,
        kind,
    };
    let first_page = PageCheckpoint {
        space_id: "octocat-hello".into(),
        issue_id: "I_1".into(),
        events: vec![
            event("E_reopen", 300, HistoryEventKind::IssueReopened),
            event("E_close", 200, HistoryEventKind::IssueClosed),
            event(
                "E_milestone",
                250,
                HistoryEventKind::MilestoneChanged {
                    from: None,
                    to: Some(MilestoneRef {
                        id: None,
                        title: "Alpha".into(),
                    }),
                },
            ),
        ],
        next_cursor: Some("CUR2".into()),
        resume_cursor: Some("CUR2".into()),
        complete: false,
    };

    let page_summary = store.checkpoint_page(&first_page).unwrap();
    let first_revision = page_summary.revision;
    let replayed = store.checkpoint_page(&first_page).unwrap();

    assert_eq!(page_summary.state, HistoryImportState::Building);
    assert_eq!(replayed.revision, first_revision);
    assert_eq!(
        store
            .pending_issue("octocat-hello")
            .unwrap()
            .unwrap()
            .cursor
            .as_deref(),
        Some("CUR2")
    );

    let imported = store
        .checkpoint_page(&PageCheckpoint {
            space_id: "octocat-hello".into(),
            issue_id: "I_1".into(),
            events: vec![],
            next_cursor: None,
            resume_cursor: Some("CUR_END".into()),
            complete: true,
        })
        .unwrap();
    let complete = store.initialize_repository(&seed).unwrap();
    let events = store.events_after("octocat-hello", 0).unwrap();

    assert_eq!(imported.state, HistoryImportState::Building);
    assert_eq!(complete.state, HistoryImportState::Complete);
    assert_eq!(complete.completed_issues, 1);
    assert_eq!(complete.verified_through, Some(500));
    assert!(store.pending_issue("octocat-hello").unwrap().is_none());
    assert_eq!(
        events
            .iter()
            .map(|event| event.provider_event_id.as_str())
            .collect::<Vec<_>>(),
        vec!["I_1:issue_created", "E_close", "E_milestone", "E_reopen"]
    );
    assert_eq!(
        events[2].kind,
        HistoryEventKind::MilestoneChanged {
            from: None,
            to: Some(MilestoneRef {
                id: None,
                title: "Alpha".into(),
            }),
        }
    );
}

#[test]
fn interrupted_import_resumes_from_the_last_checkpoint_after_reopening() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("history.sqlite3");
    let store = HistoryStore::open(&path).unwrap();
    store
        .initialize_repository(&RepositorySeed {
            space_id: "octocat-hello".into(),
            provider_repository_id: "R_repo".into(),
            verified_through: 500,
            timeline_required: true,
            issues: vec![issue("I_1", 1, 100, 400, None)],
        })
        .unwrap();
    store
        .checkpoint_page(&PageCheckpoint {
            space_id: "octocat-hello".into(),
            issue_id: "I_1".into(),
            events: vec![],
            next_cursor: Some("CUR_PAGE_2".into()),
            resume_cursor: Some("CUR_PAGE_2".into()),
            complete: false,
        })
        .unwrap();
    drop(store);

    let reopened = HistoryStore::open(&path).unwrap();
    let pending = reopened.pending_issue("octocat-hello").unwrap().unwrap();

    assert_eq!(pending.issue_id, "I_1");
    assert_eq!(pending.cursor.as_deref(), Some("CUR_PAGE_2"));
    assert_eq!(pending.cutoff, 500);
}

#[test]
fn completed_ledgers_verify_once_then_request_only_new_or_changed_issues() {
    let temp = tempfile::tempdir().unwrap();
    let store = HistoryStore::open(temp.path().join("history.sqlite3")).unwrap();
    let seed = |verified_through, issues| RepositorySeed {
        space_id: "octocat-hello".into(),
        provider_repository_id: "R_repo".into(),
        verified_through,
        timeline_required: true,
        issues,
    };

    store
        .initialize_repository(&seed(500, vec![issue("I_1", 1, 100, 400, None)]))
        .unwrap();
    let imported = store
        .checkpoint_page(&PageCheckpoint {
            space_id: "octocat-hello".into(),
            issue_id: "I_1".into(),
            events: vec![],
            next_cursor: None,
            resume_cursor: Some("CUR_END".into()),
            complete: true,
        })
        .unwrap();
    assert_eq!(imported.state, HistoryImportState::Building);
    assert!(store.pending_issue("octocat-hello").unwrap().is_none());

    let verified = store
        .initialize_repository(&seed(600, vec![issue("I_1", 1, 100, 400, None)]))
        .unwrap();
    assert_eq!(verified.state, HistoryImportState::Complete);
    assert_eq!(verified.verified_through, Some(600));
    assert!(store.pending_issue("octocat-hello").unwrap().is_none());

    let unchanged = store
        .initialize_repository(&seed(700, vec![issue("I_1", 1, 100, 400, None)]))
        .unwrap();
    assert_eq!(unchanged.state, HistoryImportState::Complete);
    assert_eq!(unchanged.verified_through, Some(700));
    assert!(store.pending_issue("octocat-hello").unwrap().is_none());

    let changed = store
        .initialize_repository(&seed(
            800,
            vec![
                issue("I_1", 1, 100, 450, None),
                issue("I_2", 2, 750, 750, None),
            ],
        ))
        .unwrap();
    assert_eq!(changed.state, HistoryImportState::Building);
    let first = store.pending_issue("octocat-hello").unwrap().unwrap();
    assert_eq!(first.issue_id, "I_1");
    assert_eq!(first.cursor.as_deref(), Some("CUR_END"));
    assert_eq!(first.cutoff, 800);

    store
        .checkpoint_page(&PageCheckpoint {
            space_id: "octocat-hello".into(),
            issue_id: "I_1".into(),
            events: vec![],
            next_cursor: None,
            resume_cursor: Some("CUR_NEW".into()),
            complete: true,
        })
        .unwrap();
    let second = store.pending_issue("octocat-hello").unwrap().unwrap();
    assert_eq!(second.issue_id, "I_2");
    assert_eq!(second.cursor, None);
}

#[test]
fn first_milestone_transition_corrects_present_day_creation_membership() {
    let store = HistoryStore::open_in_memory().unwrap();
    let initial = store
        .initialize_repository(&RepositorySeed {
            space_id: "octocat-hello".into(),
            provider_repository_id: "R_repo".into(),
            verified_through: 500,
            timeline_required: true,
            issues: vec![issue("I_1", 1, 100, 400, Some(("M_now", "Now")))],
        })
        .unwrap();
    let corrected = store
        .checkpoint_page(&PageCheckpoint {
            space_id: "octocat-hello".into(),
            issue_id: "I_1".into(),
            events: vec![HistoryEvent {
                sequence: 0,
                repository_id: "R_repo".into(),
                issue_id: "I_1".into(),
                issue_number: 1,
                provider_event_id: "E_milestone".into(),
                occurred_at: 200,
                kind: HistoryEventKind::MilestoneChanged {
                    from: None,
                    to: Some(MilestoneRef {
                        id: None,
                        title: "Now".into(),
                    }),
                },
            }],
            next_cursor: None,
            resume_cursor: Some("CUR_END".into()),
            complete: true,
        })
        .unwrap();

    let creation = store.events_after("octocat-hello", 0).unwrap().remove(0);
    assert_eq!(
        creation.kind,
        HistoryEventKind::IssueCreated { milestone: None }
    );
    assert!(creation.sequence > initial.revision);
    assert_eq!(corrected.revision, creation.sequence);
}

#[test]
fn later_snapshot_pages_cannot_hide_activity_after_the_frozen_cutoff() {
    let store = HistoryStore::open_in_memory().unwrap();
    let seed = |verified_through| RepositorySeed {
        space_id: "octocat-hello".into(),
        provider_repository_id: "R_repo".into(),
        verified_through,
        timeline_required: true,
        issues: vec![issue("I_1", 1, 600, 600, None)],
    };
    store.initialize_repository(&seed(500)).unwrap();
    assert!(store.events_after("octocat-hello", 0).unwrap().is_empty());
    store
        .checkpoint_page(&PageCheckpoint {
            space_id: "octocat-hello".into(),
            issue_id: "I_1".into(),
            events: vec![],
            next_cursor: None,
            resume_cursor: Some("CUR_BEFORE_FUTURE".into()),
            complete: true,
        })
        .unwrap();

    let catch_up = store.initialize_repository(&seed(700)).unwrap();
    let pending = store.pending_issue("octocat-hello").unwrap().unwrap();

    assert_eq!(catch_up.state, HistoryImportState::Building);
    assert_eq!(pending.cursor.as_deref(), Some("CUR_BEFORE_FUTURE"));
    assert_eq!(pending.cutoff, 700);
    assert_eq!(store.events_after("octocat-hello", 0).unwrap().len(), 1);
}

#[test]
fn rate_limit_evidence_preserves_pending_work_and_resume_time() {
    let temp = tempfile::tempdir().unwrap();
    let store = HistoryStore::open(temp.path().join("history.sqlite3")).unwrap();
    store
        .initialize_repository(&RepositorySeed {
            space_id: "octocat-hello".into(),
            provider_repository_id: "R_repo".into(),
            verified_through: 500,
            timeline_required: true,
            issues: vec![issue("I_1", 1, 100, 400, None)],
        })
        .unwrap();

    let limited = store.mark_rate_limited("octocat-hello", Some(900)).unwrap();

    assert_eq!(limited.state, HistoryImportState::RateLimited);
    assert_eq!(limited.resume_at, Some(900));
    assert_eq!(
        limited.diagnostic.as_deref(),
        Some("GitHub rate limit exceeded")
    );
    assert_eq!(
        store
            .pending_issue("octocat-hello")
            .unwrap()
            .unwrap()
            .issue_id,
        "I_1"
    );

    assert!(store.retry_repository("octocat-hello").unwrap());
    let retried = store.summary("octocat-hello").unwrap().unwrap();
    assert_eq!(retried.state, HistoryImportState::Building);
    assert_eq!(retried.resume_at, None);
    assert_eq!(retried.diagnostic, None);
    assert!(store.pending_issue("octocat-hello").unwrap().is_some());
}

#[test]
fn schema_one_creation_ledgers_reopen_for_lifecycle_backfill() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("history.sqlite3");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "PRAGMA user_version = 1;
             PRAGMA foreign_keys = ON;
             CREATE TABLE repositories (
                 space_id TEXT PRIMARY KEY,
                 provider_repository_id TEXT NOT NULL,
                 import_state TEXT NOT NULL,
                 total_issues INTEGER NOT NULL,
                 completed_issues INTEGER NOT NULL,
                 verified_through INTEGER,
                 diagnostic TEXT,
                 resume_at INTEGER
             );
             CREATE TABLE issues (
                 space_id TEXT NOT NULL REFERENCES repositories(space_id) ON DELETE CASCADE,
                 provider_issue_id TEXT NOT NULL,
                 issue_number INTEGER NOT NULL,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL,
                 milestone_id TEXT,
                 milestone_title TEXT,
                 cursor TEXT,
                 complete INTEGER NOT NULL,
                 PRIMARY KEY (space_id, provider_issue_id)
             );
             CREATE TABLE events (
                 sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                 space_id TEXT NOT NULL REFERENCES repositories(space_id) ON DELETE CASCADE,
                 provider_event_id TEXT NOT NULL,
                 provider_issue_id TEXT NOT NULL,
                 issue_number INTEGER NOT NULL,
                 occurred_at INTEGER NOT NULL,
                 payload TEXT NOT NULL,
                 UNIQUE (space_id, provider_event_id)
             );
             CREATE INDEX events_space_time
                 ON events(space_id, occurred_at, provider_event_id);
             INSERT INTO repositories VALUES
                 ('octocat-hello', 'R_repo', 'complete', 1, 1, 500, NULL, NULL);
             INSERT INTO issues VALUES
                 ('octocat-hello', 'I_1', 1, 100, 400, NULL, NULL, NULL, 1);
             INSERT INTO events (
                 space_id, provider_event_id, provider_issue_id, issue_number,
                 occurred_at, payload
             ) VALUES (
                 'octocat-hello', 'I_1:issue_created', 'I_1', 1, 100,
                 '{\"kind\":\"issue_created\",\"milestone\":null}'
             );",
        )
        .unwrap();
    drop(connection);

    let store = HistoryStore::open(&path).unwrap();
    let summary = store.summary("octocat-hello").unwrap().unwrap();
    let pending = store.pending_issue("octocat-hello").unwrap().unwrap();

    assert_eq!(summary.state, HistoryImportState::Building);
    assert_eq!(summary.completed_issues, 0);
    assert_eq!(summary.verified_through, None);
    assert_eq!(pending.issue_id, "I_1");
    assert_eq!(pending.cutoff, 500);
    assert_eq!(store.events_after("octocat-hello", 0).unwrap().len(), 1);
}

#[test]
fn failed_migration_rolls_back_without_mutating_the_existing_ledger() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("history.sqlite3");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "PRAGMA user_version = 2;
             CREATE TABLE repositories (
                 space_id TEXT PRIMARY KEY,
                 provider_repository_id TEXT NOT NULL,
                 import_state TEXT NOT NULL,
                 total_issues INTEGER NOT NULL,
                 completed_issues INTEGER NOT NULL,
                 verified_through INTEGER,
                 diagnostic TEXT,
                 resume_at INTEGER,
                 cutoff INTEGER,
                 catch_up_required INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE events (
                 sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                 space_id TEXT NOT NULL,
                 provider_event_id TEXT NOT NULL,
                 provider_issue_id TEXT NOT NULL,
                 issue_number INTEGER NOT NULL,
                 occurred_at INTEGER NOT NULL,
                 payload TEXT NOT NULL
             );
             INSERT INTO repositories VALUES
                 ('octocat-hello', 'R_repo', 'complete', 1, 1, 500, NULL, NULL, 500, 0);
             INSERT INTO events (
                 space_id, provider_event_id, provider_issue_id, issue_number,
                 occurred_at, payload
             ) VALUES (
                 'octocat-hello', 'I_1:issue_created', 'I_1', 1, 100,
                 '{\"kind\":\"issue_created\",\"milestone\":null}'
             );",
        )
        .unwrap();
    drop(connection);

    assert!(HistoryStore::open(&path).is_err());

    let connection = rusqlite::Connection::open(&path).unwrap();
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    let events: i64 = connection
        .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 2);
    assert_eq!(events, 1);
}
