use std::{sync::Arc, time::Duration};

use chrono::Utc;
use stellr_core::{
    HistoryImportState, HistoryPageRequest, HistorySummary, Model, Provider, SpaceModel, derive,
};
use stellr_github::cache::{Cache, Snapshot};
use stellr_history::RepositorySeed;

use crate::{spaces::SpaceEntry, state::AppState};

#[derive(Clone)]
pub struct PollingControl {
    intervals: tokio::sync::watch::Sender<Duration>,
    focused: Duration,
    background: Duration,
}

impl PollingControl {
    pub fn fixed(interval: Duration) -> Self {
        Self::focus_aware(interval, interval)
    }

    pub fn focus_aware(focused: Duration, background: Duration) -> Self {
        let (intervals, _) = tokio::sync::watch::channel(focused);
        Self {
            intervals,
            focused,
            background,
        }
    }

    pub fn set_focused(&self, focused: bool) {
        let next = if focused {
            self.focused
        } else {
            self.background
        };
        if *self.intervals.borrow() != next {
            self.intervals.send_replace(next);
        }
    }

    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<Duration> {
        self.intervals.subscribe()
    }
}

pub fn spawn_poller(
    state: Arc<AppState>,
    provider: Arc<dyn Provider + Send + Sync>,
    cache: Cache,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    let control = PollingControl::fixed(interval);
    let intervals = control.subscribe();
    tokio::spawn(async move {
        let _control = control;
        run_poller(state, provider, cache, intervals).await;
    })
}

pub fn spawn_controlled_poller(
    state: Arc<AppState>,
    provider: Arc<dyn Provider + Send + Sync>,
    cache: Cache,
    intervals: tokio::sync::watch::Receiver<Duration>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(run_poller(state, provider, cache, intervals))
}

async fn run_poller(
    state: Arc<AppState>,
    provider: Arc<dyn Provider + Send + Sync>,
    cache: Cache,
    mut intervals: tokio::sync::watch::Receiver<Duration>,
) {
    sync_spaces(&state, provider.as_ref(), &cache).await;
    let mut interval_updates_open = true;
    loop {
        let interval = *intervals.borrow_and_update();
        let deadline = tokio::time::sleep(interval);
        tokio::pin!(deadline);
        let should_sync = tokio::select! {
            _ = &mut deadline => true,
            _ = state.refresh.notified() => true,
            update = intervals.changed(), if interval_updates_open => {
                if update.is_err() {
                    interval_updates_open = false;
                }
                false
            }
        };
        if should_sync {
            sync_spaces(&state, provider.as_ref(), &cache).await;
        }
    }
}

async fn sync_spaces(state: &AppState, provider: &(dyn Provider + Send + Sync), cache: &Cache) {
    let entries = state.spaces.lock().await.entries().to_vec();
    let mut spaces = Vec::with_capacity(entries.len());
    for entry in &entries {
        spaces.push(sync_space(entry, provider, cache, &state.history).await);
    }
    state.hub.send_replace(Model { spaces });
    for entry in &entries {
        import_pending_history(state, provider, entry).await;
    }
}

async fn import_pending_history(
    state: &AppState,
    provider: &(dyn Provider + Send + Sync),
    entry: &SpaceEntry,
) {
    loop {
        let pending = match state.history.pending_issue(&entry.id) {
            Ok(Some(pending)) => pending,
            Ok(None) => return,
            Err(error) => {
                publish_history_summary(state, &entry.id, failed_history(error.to_string()));
                return;
            }
        };
        let request = HistoryPageRequest {
            issue_id: pending.issue_id.clone(),
            issue_number: pending.issue_number,
            cursor: pending.cursor.clone(),
            cutoff: pending.cutoff,
        };
        let page = match provider.fetch_history_page(&entry.repo, &request).await {
            Ok(page) => page,
            Err(error) => {
                let summary = state
                    .history
                    .mark_failed(&entry.id, error.to_string())
                    .unwrap_or_else(|store_error| failed_history(store_error.to_string()));
                publish_history_summary(state, &entry.id, summary);
                return;
            }
        };
        let summary = match state
            .history
            .checkpoint_page(&stellr_history::PageCheckpoint {
                space_id: entry.id.clone(),
                issue_id: pending.issue_id,
                events: page.events,
                next_cursor: page.next_cursor,
                complete: page.complete,
            }) {
            Ok(summary) => summary,
            Err(error) => {
                let summary = state
                    .history
                    .mark_failed(&entry.id, error.to_string())
                    .unwrap_or_else(|store_error| failed_history(store_error.to_string()));
                publish_history_summary(state, &entry.id, summary);
                return;
            }
        };
        publish_history_summary(state, &entry.id, summary);
        tokio::task::yield_now().await;
    }
}

fn publish_history_summary(state: &AppState, space_id: &str, summary: HistorySummary) {
    let mut model = state.hub.borrow().clone();
    let Some(space) = model.spaces.iter_mut().find(|space| space.id == space_id) else {
        return;
    };
    space.history = summary;
    state.hub.send_replace(model);
}

async fn sync_space(
    entry: &SpaceEntry,
    provider: &(dyn Provider + Send + Sync),
    cache: &Cache,
    history_store: &stellr_history::HistoryStore,
) -> SpaceModel {
    match provider.fetch_snapshot(&entry.repo).await {
        Ok(snapshot) => {
            let synced_at = Utc::now().timestamp();
            // A successful provider sync is fresh even if its fallback cache cannot be updated.
            let _ = cache.store(
                &entry.repo,
                &Snapshot {
                    issues: snapshot.issues.clone(),
                    synced_at,
                },
            );
            let history = history_after_snapshot(entry, &snapshot, history_store, synced_at);
            model(
                entry,
                snapshot.issues,
                Some(synced_at),
                false,
                None,
                history,
            )
        }
        Err(error) => {
            let snapshot = cache.load(&entry.repo);
            let (issues, synced_at) = snapshot
                .map(|snapshot| (snapshot.issues, Some(snapshot.synced_at)))
                .unwrap_or_default();
            let history = history_store
                .summary(&entry.id)
                .unwrap_or_else(|history_error| Some(failed_history(history_error.to_string())))
                .unwrap_or_default();
            model(
                entry,
                issues,
                synced_at,
                true,
                Some(error.to_string()),
                history,
            )
        }
    }
}

fn history_after_snapshot(
    entry: &SpaceEntry,
    snapshot: &stellr_core::ProviderSnapshot,
    store: &stellr_history::HistoryStore,
    verified_through: i64,
) -> HistorySummary {
    let Some(repository_id) = snapshot.repository_id.as_ref() else {
        return store
            .summary(&entry.id)
            .unwrap_or_else(|error| Some(failed_history(error.to_string())))
            .unwrap_or_default();
    };
    if snapshot.history.len() != snapshot.issues.len() {
        return failed_history(format!(
            "provider returned history metadata for {}/{} issues",
            snapshot.history.len(),
            snapshot.issues.len()
        ));
    }
    store
        .initialize_repository(&RepositorySeed {
            space_id: entry.id.clone(),
            provider_repository_id: repository_id.clone(),
            verified_through,
            timeline_required: true,
            issues: snapshot.history.clone(),
        })
        .unwrap_or_else(|error| failed_history(error.to_string()))
}

fn failed_history(diagnostic: String) -> HistorySummary {
    HistorySummary {
        state: HistoryImportState::Failed,
        diagnostic: Some(diagnostic),
        ..HistorySummary::default()
    }
}

fn model(
    entry: &SpaceEntry,
    issues: Vec<stellr_core::RawIssue>,
    synced_at: Option<i64>,
    stale: bool,
    error: Option<String>,
    history: HistorySummary,
) -> SpaceModel {
    SpaceModel {
        id: entry.id.clone(),
        repo: entry.repo.slug(),
        name: entry.repo.name.clone(),
        stars: derive(&issues),
        synced_at,
        stale,
        error,
        history,
    }
}
