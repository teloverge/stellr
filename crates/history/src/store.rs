use std::{
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

use rusqlite::{Connection, OptionalExtension, params};
use stellr_core::{
    HistoryEvent, HistoryEventKind, HistoryImportState, HistorySummary, IssueSyncMetadata,
};

const SCHEMA_VERSION: i64 = 3;

#[derive(Clone, Copy)]
enum StoredImportState {
    Unavailable,
    Building,
    Complete,
    Delayed,
    RateLimited,
    Failed,
}

impl StoredImportState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Building => "building",
            Self::Complete => "complete",
            Self::Delayed => "delayed",
            Self::RateLimited => "rate_limited",
            Self::Failed => "failed",
        }
    }

    fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "unavailable" => Ok(Self::Unavailable),
            "building" => Ok(Self::Building),
            "complete" => Ok(Self::Complete),
            "delayed" => Ok(Self::Delayed),
            "rate_limited" => Ok(Self::RateLimited),
            "failed" => Ok(Self::Failed),
            _ => Err(StoreError::Invalid(format!(
                "unknown history import state: {value}"
            ))),
        }
    }
}

impl From<StoredImportState> for HistoryImportState {
    fn from(value: StoredImportState) -> Self {
        match value {
            StoredImportState::Unavailable => Self::Unavailable,
            StoredImportState::Building => Self::Building,
            StoredImportState::Complete => Self::Complete,
            StoredImportState::Delayed => Self::Delayed,
            StoredImportState::RateLimited => Self::RateLimited,
            StoredImportState::Failed => Self::Failed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositorySeed {
    pub space_id: String,
    pub provider_repository_id: String,
    pub verified_through: i64,
    pub timeline_required: bool,
    pub issues: Vec<IssueSyncMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingHistoryIssue {
    pub issue_id: String,
    pub issue_number: u64,
    pub cursor: Option<String>,
    pub cutoff: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageCheckpoint {
    pub space_id: String,
    pub issue_id: String,
    pub events: Vec<HistoryEvent>,
    pub next_cursor: Option<String>,
    pub resume_cursor: Option<String>,
    pub complete: bool,
}

#[derive(Clone)]
pub struct HistoryStore {
    connection: Arc<Mutex<Connection>>,
}

impl HistoryStore {
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let mut connection = Connection::open_in_memory()?;
        connection.pragma_update(None, "foreign_keys", true)?;
        migrate(&mut connection)?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        migrate(&mut connection)?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub fn initialize_repository(
        &self,
        seed: &RepositorySeed,
    ) -> Result<HistorySummary, StoreError> {
        validate_seed(seed)?;
        let mut issues = seed.issues.clone();
        issues.sort_by(|left, right| {
            (left.created_at, HistoryEvent::creation_id(&left.issue_id))
                .cmp(&(right.created_at, HistoryEvent::creation_id(&right.issue_id)))
        });
        let timeline_required = seed.timeline_required && !issues.is_empty();
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;

        transaction.execute(
            "INSERT INTO repositories (
                 space_id, provider_repository_id, import_state, total_issues,
                 completed_issues, verified_through, diagnostic, resume_at, cutoff,
                 catch_up_required
             ) VALUES (?1, ?2, ?3, 0, 0, ?4, NULL, NULL, ?5, ?6)
             ON CONFLICT(space_id) DO NOTHING",
            params![
                seed.space_id,
                seed.provider_repository_id,
                if timeline_required {
                    StoredImportState::Building.as_str()
                } else {
                    StoredImportState::Complete.as_str()
                },
                (!timeline_required).then_some(seed.verified_through),
                seed.verified_through,
                timeline_required,
            ],
        )?;
        let stored_repository_id: String = transaction.query_row(
            "SELECT provider_repository_id FROM repositories WHERE space_id = ?1",
            params![seed.space_id],
            |row| row.get(0),
        )?;
        if stored_repository_id != seed.provider_repository_id {
            return Err(StoreError::Invalid(
                "provider repository identity changed for an existing space".into(),
            ));
        }

        let (catch_up_required, pending_count): (bool, i64) = transaction.query_row(
            "SELECT r.catch_up_required,
                    SUM(CASE WHEN i.complete = 0 THEN 1 ELSE 0 END)
             FROM repositories r
             LEFT JOIN issues i ON i.space_id = r.space_id
             WHERE r.space_id = ?1
             GROUP BY r.space_id",
            params![seed.space_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if catch_up_required && pending_count == 0 {
            transaction.execute(
                "UPDATE repositories SET catch_up_required = 0 WHERE space_id = ?1",
                params![seed.space_id],
            )?;
        }

        transaction.execute(
            "UPDATE repositories
             SET cutoff = ?2, diagnostic = NULL, resume_at = NULL
             WHERE space_id = ?1",
            params![seed.space_id, seed.verified_through],
        )?;

        let mut changed = false;
        for issue in issues {
            let issue_number = i64::try_from(issue.number)?;
            // Later pages can be observed after the response-backed cutoff. Keeping
            // their baseline at the cutoff guarantees the confirming snapshot will
            // queue any activity that happened while pagination was in flight.
            let observed_updated_at = issue.updated_at.min(seed.verified_through);
            let (milestone_id, milestone_title) = issue
                .milestone
                .as_ref()
                .map(|milestone| (milestone.id.as_deref(), Some(milestone.title.as_str())))
                .unwrap_or((None, None));
            let stored_updated_at = transaction
                .query_row(
                    "SELECT updated_at FROM issues
                     WHERE space_id = ?1 AND provider_issue_id = ?2",
                    params![seed.space_id, issue.issue_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?;
            match stored_updated_at {
                None => {
                    changed |= timeline_required;
                    transaction.execute(
                        "INSERT INTO issues (
                             space_id, provider_issue_id, issue_number, created_at, updated_at,
                             milestone_id, milestone_title, cursor, complete
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8)",
                        params![
                            seed.space_id,
                            issue.issue_id,
                            issue_number,
                            issue.created_at,
                            observed_updated_at,
                            milestone_id,
                            milestone_title,
                            !timeline_required
                        ],
                    )?;
                }
                Some(stored_updated_at) => {
                    let issue_changed =
                        timeline_required && observed_updated_at > stored_updated_at;
                    changed |= issue_changed;
                    transaction.execute(
                        "UPDATE issues
                         SET issue_number = ?3,
                             created_at = ?4,
                             updated_at = MAX(updated_at, ?5),
                             milestone_id = ?6,
                             milestone_title = ?7,
                             complete = CASE WHEN ?8 THEN 0 ELSE complete END
                         WHERE space_id = ?1 AND provider_issue_id = ?2",
                        params![
                            seed.space_id,
                            issue.issue_id,
                            issue_number,
                            issue.created_at,
                            observed_updated_at,
                            milestone_id,
                            milestone_title,
                            issue_changed
                        ],
                    )?;
                }
            }

            if issue.created_at <= seed.verified_through {
                let kind = HistoryEventKind::IssueCreated {
                    milestone: issue.milestone,
                };
                transaction.execute(
                    "INSERT OR IGNORE INTO events (
                         space_id, provider_event_id, provider_issue_id, issue_number,
                         occurred_at, payload
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        seed.space_id,
                        HistoryEvent::creation_id(&issue.issue_id),
                        issue.issue_id,
                        issue_number,
                        issue.created_at,
                        serde_json::to_string(&kind)?
                    ],
                )?;
            }
        }

        if changed {
            transaction.execute(
                "UPDATE repositories SET catch_up_required = 1 WHERE space_id = ?1",
                params![seed.space_id],
            )?;
        }
        let (completed, total, catch_up_required): (i64, i64, bool) = transaction.query_row(
            "SELECT SUM(CASE WHEN i.complete = 1 THEN 1 ELSE 0 END),
                        COUNT(i.provider_issue_id), r.catch_up_required
                 FROM repositories r
                 LEFT JOIN issues i ON i.space_id = r.space_id
                 WHERE r.space_id = ?1
                 GROUP BY r.space_id",
            params![seed.space_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let complete = completed == total && !catch_up_required;
        transaction.execute(
            "UPDATE repositories
             SET import_state = ?2,
                 total_issues = ?3,
                 completed_issues = ?4,
                 verified_through = CASE WHEN ?5 THEN cutoff ELSE verified_through END
             WHERE space_id = ?1",
            params![
                seed.space_id,
                if complete {
                    StoredImportState::Complete.as_str()
                } else {
                    StoredImportState::Building.as_str()
                },
                total,
                completed,
                complete
            ],
        )?;

        transaction.commit()?;
        drop(connection);
        self.summary(&seed.space_id)?.ok_or_else(|| {
            StoreError::Invalid("initialized repository is missing from the ledger".into())
        })
    }

    pub fn summary(&self, space_id: &str) -> Result<Option<HistorySummary>, StoreError> {
        let connection = self.connection()?;
        summary(&connection, space_id)
    }

    pub fn events_after(
        &self,
        space_id: &str,
        sequence: u64,
    ) -> Result<Vec<HistoryEvent>, StoreError> {
        let sequence = i64::try_from(sequence)?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT e.sequence, r.provider_repository_id, e.provider_issue_id,
                    e.issue_number, e.provider_event_id, e.occurred_at, e.payload
             FROM events e
             JOIN repositories r ON r.space_id = e.space_id
             WHERE e.space_id = ?1 AND e.sequence > ?2
             ORDER BY e.occurred_at, e.provider_event_id",
        )?;
        let rows = statement.query_map(params![space_id, sequence], |row| {
            let sequence: i64 = row.get(0)?;
            let issue_number: i64 = row.get(3)?;
            let payload: String = row.get(6)?;
            Ok((
                sequence,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                issue_number,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                payload,
            ))
        })?;
        let mut events = Vec::new();
        for row in rows {
            let (
                sequence,
                repository_id,
                issue_id,
                issue_number,
                provider_event_id,
                occurred_at,
                payload,
            ) = row?;
            events.push(HistoryEvent {
                sequence: u64::try_from(sequence)?,
                repository_id,
                issue_id,
                issue_number: u64::try_from(issue_number)?,
                provider_event_id,
                occurred_at,
                kind: serde_json::from_str(&payload)?,
            });
        }
        Ok(events)
    }

    pub fn pending_issue(&self, space_id: &str) -> Result<Option<PendingHistoryIssue>, StoreError> {
        let connection = self.connection()?;
        let row = connection
            .query_row(
                "SELECT i.provider_issue_id, i.issue_number, i.cursor, r.cutoff
                 FROM issues i
                 JOIN repositories r ON r.space_id = i.space_id
                 WHERE i.space_id = ?1 AND i.complete = 0
                 ORDER BY i.issue_number
                 LIMIT 1",
                params![space_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(issue_id, issue_number, cursor, cutoff)| {
            Ok(PendingHistoryIssue {
                issue_id,
                issue_number: u64::try_from(issue_number)?,
                cursor,
                cutoff,
            })
        })
        .transpose()
    }

    pub fn checkpoint_page(
        &self,
        checkpoint: &PageCheckpoint,
    ) -> Result<HistorySummary, StoreError> {
        if checkpoint.space_id.trim().is_empty() || checkpoint.issue_id.trim().is_empty() {
            return Err(StoreError::Invalid(
                "checkpoint identities must not be empty".into(),
            ));
        }
        if checkpoint.complete && checkpoint.next_cursor.is_some() {
            return Err(StoreError::Invalid(
                "a complete history page cannot retain a next cursor".into(),
            ));
        }
        if !checkpoint.complete && checkpoint.next_cursor.is_none() {
            return Err(StoreError::Invalid(
                "an incomplete history page requires a next cursor".into(),
            ));
        }

        let mut events = checkpoint.events.clone();
        events.sort();
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        let (repository_id, cutoff): (String, i64) = transaction.query_row(
            "SELECT provider_repository_id, cutoff
             FROM repositories WHERE space_id = ?1",
            params![checkpoint.space_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let (issue_number, issue_created_at): (i64, i64) = transaction.query_row(
            "SELECT issue_number, created_at FROM issues
             WHERE space_id = ?1 AND provider_issue_id = ?2",
            params![checkpoint.space_id, checkpoint.issue_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let issue_number = u64::try_from(issue_number)?;

        for event in events {
            if event.repository_id != repository_id
                || event.issue_id != checkpoint.issue_id
                || event.issue_number != issue_number
            {
                return Err(StoreError::Invalid(
                    "history event does not match its repository checkpoint".into(),
                ));
            }
            if event.occurred_at > cutoff {
                return Err(StoreError::Invalid(
                    "history event is later than the repository cutoff".into(),
                ));
            }
            transaction.execute(
                "INSERT OR IGNORE INTO events (
                     space_id, provider_event_id, provider_issue_id, issue_number,
                     occurred_at, payload
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    checkpoint.space_id,
                    event.provider_event_id,
                    checkpoint.issue_id,
                    i64::try_from(issue_number)?,
                    event.occurred_at,
                    serde_json::to_string(&event.kind)?
                ],
            )?;
        }
        reconcile_creation_milestone(
            &transaction,
            &checkpoint.space_id,
            &checkpoint.issue_id,
            issue_created_at,
        )?;

        let stored_cursor = if checkpoint.complete {
            checkpoint.resume_cursor.as_ref()
        } else {
            checkpoint.next_cursor.as_ref()
        };
        transaction.execute(
            "UPDATE issues
             SET cursor = ?3, complete = ?4
             WHERE space_id = ?1 AND provider_issue_id = ?2",
            params![
                checkpoint.space_id,
                checkpoint.issue_id,
                stored_cursor,
                checkpoint.complete
            ],
        )?;
        let (completed, total, catch_up_required): (i64, i64, bool) = transaction.query_row(
            "SELECT SUM(CASE WHEN i.complete = 1 THEN 1 ELSE 0 END), COUNT(*),
                    r.catch_up_required
             FROM repositories r
             JOIN issues i ON i.space_id = r.space_id
             WHERE r.space_id = ?1",
            params![checkpoint.space_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let repository_complete = completed == total && !catch_up_required;
        transaction.execute(
            "UPDATE repositories
             SET import_state = ?2,
                 completed_issues = ?3,
                 verified_through = CASE WHEN ?4 IS NOT NULL THEN ?4 ELSE verified_through END,
                 diagnostic = NULL,
                 resume_at = NULL
             WHERE space_id = ?1",
            params![
                checkpoint.space_id,
                if repository_complete {
                    StoredImportState::Complete.as_str()
                } else {
                    StoredImportState::Building.as_str()
                },
                completed,
                repository_complete.then_some(cutoff)
            ],
        )?;
        transaction.commit()?;
        drop(connection);
        self.summary(&checkpoint.space_id)?.ok_or_else(|| {
            StoreError::Invalid("checkpoint repository disappeared from the ledger".into())
        })
    }

    pub fn mark_failed(
        &self,
        space_id: &str,
        diagnostic: impl Into<String>,
    ) -> Result<HistorySummary, StoreError> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE repositories
             SET import_state = ?2, diagnostic = ?3, resume_at = NULL
             WHERE space_id = ?1",
            params![
                space_id,
                StoredImportState::Failed.as_str(),
                diagnostic.into()
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::Invalid(
                "cannot fail unknown history repository".into(),
            ));
        }
        drop(connection);
        self.summary(space_id)?.ok_or_else(|| {
            StoreError::Invalid("failed repository disappeared from the ledger".into())
        })
    }

    pub fn mark_rate_limited(
        &self,
        space_id: &str,
        resume_at: Option<i64>,
    ) -> Result<HistorySummary, StoreError> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE repositories
             SET import_state = ?2,
                 diagnostic = 'GitHub rate limit exceeded',
                 resume_at = ?3
             WHERE space_id = ?1",
            params![space_id, StoredImportState::RateLimited.as_str(), resume_at],
        )?;
        if changed == 0 {
            return Err(StoreError::Invalid(
                "cannot rate-limit unknown history repository".into(),
            ));
        }
        drop(connection);
        self.summary(space_id)?.ok_or_else(|| {
            StoreError::Invalid("rate-limited repository disappeared from the ledger".into())
        })
    }

    pub fn retry_repository(&self, space_id: &str) -> Result<bool, StoreError> {
        let connection = self.connection()?;
        Ok(connection.execute(
            "UPDATE repositories
             SET import_state = ?2, diagnostic = NULL, resume_at = NULL
             WHERE space_id = ?1
               AND import_state IN (?3, ?4, ?5)",
            params![
                space_id,
                StoredImportState::Building.as_str(),
                StoredImportState::RateLimited.as_str(),
                StoredImportState::Delayed.as_str(),
                StoredImportState::Failed.as_str()
            ],
        )? > 0)
    }

    pub fn remove_repository(&self, space_id: &str) -> Result<bool, StoreError> {
        let connection = self.connection()?;
        Ok(connection.execute(
            "DELETE FROM repositories WHERE space_id = ?1",
            params![space_id],
        )? > 0)
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>, StoreError> {
        self.connection
            .lock()
            .map_err(|_| StoreError::Invalid("history connection lock is poisoned".into()))
    }
}

fn reconcile_creation_milestone(
    transaction: &rusqlite::Transaction<'_>,
    space_id: &str,
    issue_id: &str,
    issue_created_at: i64,
) -> Result<(), StoreError> {
    let mut statement = transaction.prepare(
        "SELECT occurred_at, payload
         FROM events
         WHERE space_id = ?1 AND provider_issue_id = ?2
         ORDER BY occurred_at, provider_event_id",
    )?;
    let rows = statement.query_map(params![space_id, issue_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut initial_milestone = None;
    let mut saw_milestone_transition = false;
    for row in rows {
        let (occurred_at, payload) = row?;
        let kind: HistoryEventKind = serde_json::from_str(&payload)?;
        let HistoryEventKind::MilestoneChanged { from, to } = kind else {
            continue;
        };
        initial_milestone = if from.is_some() {
            from
        } else if occurred_at == issue_created_at {
            to
        } else {
            None
        };
        saw_milestone_transition = true;
        break;
    }
    drop(statement);
    if saw_milestone_transition {
        let payload = serde_json::to_string(&HistoryEventKind::IssueCreated {
            milestone: initial_milestone,
        })?;
        transaction.execute(
            "UPDATE events
             SET sequence = (SELECT COALESCE(MAX(sequence), 0) + 1 FROM events),
                 payload = ?3
             WHERE space_id = ?1 AND provider_event_id = ?2 AND payload <> ?3",
            params![space_id, HistoryEvent::creation_id(issue_id), payload],
        )?;
    }
    Ok(())
}

fn validate_seed(seed: &RepositorySeed) -> Result<(), StoreError> {
    if seed.space_id.trim().is_empty() || seed.provider_repository_id.trim().is_empty() {
        return Err(StoreError::Invalid(
            "repository identities must not be empty".into(),
        ));
    }
    for issue in &seed.issues {
        if issue.issue_id.trim().is_empty() {
            return Err(StoreError::Invalid(
                "provider issue identity must not be empty".into(),
            ));
        }
        let _ = i64::try_from(issue.number)?;
    }
    Ok(())
}

fn migrate(connection: &mut Connection) -> Result<(), StoreError> {
    let mut version =
        connection.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?;
    if version > SCHEMA_VERSION {
        return Err(StoreError::UnsupportedSchema(version));
    }
    if version == 0 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "CREATE TABLE repositories (
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
             ON events(space_id, occurred_at, provider_event_id);",
        )?;
        transaction.pragma_update(None, "user_version", 1)?;
        transaction.commit()?;
        version = 1;
    }
    if version == 1 {
        let transaction = connection.transaction()?;
        transaction.execute_batch("ALTER TABLE repositories ADD COLUMN cutoff INTEGER;")?;
        transaction.execute_batch(
            "UPDATE repositories SET cutoff = verified_through WHERE cutoff IS NULL;
             UPDATE issues SET complete = 0;
             UPDATE repositories
             SET import_state = CASE WHEN total_issues = 0 THEN 'complete' ELSE 'building' END,
                 completed_issues = 0,
                 verified_through = CASE WHEN total_issues = 0 THEN cutoff ELSE NULL END,
                 diagnostic = NULL,
                 resume_at = NULL;",
        )?;
        transaction.pragma_update(None, "user_version", 2)?;
        transaction.commit()?;
        version = 2;
    }
    if version == 2 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "ALTER TABLE repositories
                 ADD COLUMN catch_up_required INTEGER NOT NULL DEFAULT 0;
             UPDATE repositories
             SET catch_up_required = CASE WHEN total_issues > 0 THEN 1 ELSE 0 END,
                 import_state = CASE WHEN total_issues > 0 THEN 'building' ELSE import_state END;",
        )?;
        transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        transaction.commit()?;
    }
    Ok(())
}

fn summary(connection: &Connection, space_id: &str) -> Result<Option<HistorySummary>, StoreError> {
    let row = connection
        .query_row(
            "SELECT r.import_state, r.completed_issues, r.total_issues,
                    MIN(e.occurred_at), r.verified_through, MAX(e.sequence),
                    r.diagnostic, r.resume_at
             FROM repositories r
             LEFT JOIN events e ON e.space_id = r.space_id
             WHERE r.space_id = ?1
             GROUP BY r.space_id",
            params![space_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                ))
            },
        )
        .optional()?;
    let Some((
        state,
        completed_issues,
        total_issues,
        earliest_event_at,
        verified_through,
        revision,
        diagnostic,
        resume_at,
    )) = row
    else {
        return Ok(None);
    };
    Ok(Some(HistorySummary {
        state: parse_state(&state)?,
        completed_issues: u64::try_from(completed_issues)?,
        total_issues: u64::try_from(total_issues)?,
        earliest_event_at,
        verified_through,
        revision: u64::try_from(revision.unwrap_or(0))?,
        diagnostic,
        resume_at,
    }))
}

fn parse_state(state: &str) -> Result<HistoryImportState, StoreError> {
    StoredImportState::parse(state).map(Into::into)
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("history database I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("history database failed: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("history event payload failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("history integer is outside the supported range: {0}")]
    Integer(#[from] std::num::TryFromIntError),
    #[error("unsupported history schema version {0}")]
    UnsupportedSchema(i64),
    #[error("invalid history data: {0}")]
    Invalid(String),
}
