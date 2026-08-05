use std::{
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

use rusqlite::{Connection, OptionalExtension, params};
use stellr_core::{
    HistoryEvent, HistoryEventKind, HistoryImportState, HistorySummary, IssueSyncMetadata,
};

const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositorySeed {
    pub space_id: String,
    pub provider_repository_id: String,
    pub verified_through: i64,
    pub issues: Vec<IssueSyncMetadata>,
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
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;

        transaction.execute(
            "INSERT INTO repositories (
                 space_id, provider_repository_id, import_state, total_issues,
                 completed_issues, verified_through, diagnostic, resume_at
             ) VALUES (?1, ?2, 'complete', ?3, ?3, ?4, NULL, NULL)
             ON CONFLICT(space_id) DO UPDATE SET
                 provider_repository_id = excluded.provider_repository_id,
                 import_state = excluded.import_state,
                 total_issues = excluded.total_issues,
                 completed_issues = excluded.completed_issues,
                 verified_through = excluded.verified_through,
                 diagnostic = NULL,
                 resume_at = NULL",
            params![
                seed.space_id,
                seed.provider_repository_id,
                total_issues,
                seed.verified_through
            ],
        )?;

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
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, 1)
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
                    milestone_title
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
    let version =
        connection.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?;
    if version == SCHEMA_VERSION {
        return Ok(());
    }
    if version != 0 {
        return Err(StoreError::UnsupportedSchema(version));
    }
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
    transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    transaction.commit()?;
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
