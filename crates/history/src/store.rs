use std::{
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

use rusqlite::{Connection, OptionalExtension, params};
use stellr_core::{
    HistoryEvent, HistoryEventKind, HistoryImportState, HistorySummary, IssueSyncMetadata,
};

const SCHEMA_VERSION: i64 = 2;

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
        let total_issues = i64::try_from(issues.len())?;
        let timeline_required = seed.timeline_required && total_issues > 0;
        let import_state = if timeline_required {
            "building"
        } else {
            "complete"
        };
        let completed_issues = if timeline_required { 0 } else { total_issues };
        let verified_through = (!timeline_required).then_some(seed.verified_through);
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;

        transaction.execute(
            "INSERT INTO repositories (
                 space_id, provider_repository_id, import_state, total_issues,
                 completed_issues, verified_through, diagnostic, resume_at, cutoff
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, ?7)
             ON CONFLICT(space_id) DO NOTHING",
            params![
                seed.space_id,
                seed.provider_repository_id,
                import_state,
                total_issues,
                completed_issues,
                verified_through,
                seed.verified_through
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

        for issue in issues {
            let issue_number = i64::try_from(issue.number)?;
            let (milestone_id, milestone_title) = issue
                .milestone
                .as_ref()
                .map(|milestone| (Some(milestone.id.as_str()), Some(milestone.title.as_str())))
                .unwrap_or((None, None));
            transaction.execute(
                "INSERT INTO issues (
                     space_id, provider_issue_id, issue_number, created_at, updated_at,
                     milestone_id, milestone_title, cursor, complete
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8)
                 ON CONFLICT(space_id, provider_issue_id) DO UPDATE SET
                     issue_number = excluded.issue_number,
                     created_at = excluded.created_at,
                     updated_at = excluded.updated_at,
                     milestone_id = excluded.milestone_id,
                     milestone_title = excluded.milestone_title",
                params![
                    seed.space_id,
                    issue.issue_id,
                    issue_number,
                    issue.created_at,
                    issue.updated_at,
                    milestone_id,
                    milestone_title,
                    !timeline_required
                ],
            )?;

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
        let issue_number: i64 = transaction.query_row(
            "SELECT issue_number FROM issues
             WHERE space_id = ?1 AND provider_issue_id = ?2",
            params![checkpoint.space_id, checkpoint.issue_id],
            |row| row.get(0),
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

        transaction.execute(
            "UPDATE issues
             SET cursor = ?3, complete = ?4
             WHERE space_id = ?1 AND provider_issue_id = ?2",
            params![
                checkpoint.space_id,
                checkpoint.issue_id,
                checkpoint.next_cursor,
                checkpoint.complete
            ],
        )?;
        let (completed, total): (i64, i64) = transaction.query_row(
            "SELECT SUM(CASE WHEN complete = 1 THEN 1 ELSE 0 END), COUNT(*)
             FROM issues WHERE space_id = ?1",
            params![checkpoint.space_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let repository_complete = completed == total;
        transaction.execute(
            "UPDATE repositories
             SET import_state = ?2,
                 completed_issues = ?3,
                 verified_through = ?4,
                 diagnostic = NULL,
                 resume_at = NULL
             WHERE space_id = ?1",
            params![
                checkpoint.space_id,
                if repository_complete {
                    "complete"
                } else {
                    "building"
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
             SET import_state = 'failed', diagnostic = ?2, resume_at = NULL
             WHERE space_id = ?1",
            params![space_id, diagnostic.into()],
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
    match state {
        "unavailable" => Ok(HistoryImportState::Unavailable),
        "building" => Ok(HistoryImportState::Building),
        "complete" => Ok(HistoryImportState::Complete),
        "delayed" => Ok(HistoryImportState::Delayed),
        "rate_limited" => Ok(HistoryImportState::RateLimited),
        "failed" => Ok(HistoryImportState::Failed),
        _ => Err(StoreError::Invalid(format!(
            "unknown history import state: {state}"
        ))),
    }
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
