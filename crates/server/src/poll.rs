use std::{sync::Arc, time::Duration};

use chrono::Utc;
use stellr_core::{
    HistoryImportState, HistoryPageRequest, HistorySummary, Model, Provider, ProviderError,
    ProviderSnapshot, SpaceModel, derive,
};
use stellr_github::cache::{Cache, Snapshot};
use stellr_history::RepositorySeed;

use crate::{spaces::SpaceEntry, state::AppState};

const CONSERVATIVE_RATE_LIMIT_BACKOFF: i64 = 60;

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
    let mut results = Vec::with_capacity(entries.len());
    for entry in &entries {
        results.push(provider.fetch_snapshot(&entry.repo).await);
    }
    let publication_generations = publication_generations(&results);
    let mut pending = Some(results);
    let mut publish = || {
        let spaces = entries
            .iter()
            .zip(pending.take().expect("provider commit runs once"))
            .map(|(entry, result)| sync_result(entry, result, provider, cache, &state.history))
            .collect();
        state.hub.send_replace(Model { spaces });
    };
    let published = provider.commit_if_current(&publication_generations, &mut publish);

    if !published {
        let mut publish_safe_fallback = || {
            let spaces = entries
                .iter()
                .map(|entry| {
                    cached_model(
                        entry,
                        provider,
                        cache,
                        &state.history,
                        ProviderError::Superseded,
                    )
                })
                .collect();
            state.hub.send_replace(Model { spaces });
        };
        let committed = provider.commit_if_current(&[], &mut publish_safe_fallback);
        debug_assert!(committed, "an empty fallback batch must be current");
        return;
    }

    for entry in &entries {
        import_pending_history(state, provider, entry).await;
        if needs_catch_up(&state.history, &entry.id) {
            let result = provider.fetch_snapshot(&entry.repo).await;
            let generations = result
                .as_ref()
                .ok()
                .and_then(ProviderSnapshot::publication_generation)
                .into_iter()
                .collect::<Vec<_>>();
            let mut pending = Some(result);
            let mut publish = || {
                let space = sync_result(
                    entry,
                    pending.take().expect("provider commit runs once"),
                    provider,
                    cache,
                    &state.history,
                );
                publish_space(state, space);
            };
            if provider.commit_if_current(&generations, &mut publish) {
                import_pending_history(state, provider, entry).await;
            }
        }
    }
}

fn publication_generations(results: &[Result<ProviderSnapshot, ProviderError>]) -> Vec<u64> {
    results
        .iter()
        .filter_map(|result| {
            result
                .as_ref()
                .ok()
                .and_then(ProviderSnapshot::publication_generation)
        })
        .collect()
}

fn needs_catch_up(history: &stellr_history::HistoryStore, space_id: &str) -> bool {
    history
        .summary(space_id)
        .ok()
        .flatten()
        .is_some_and(|summary| {
            summary.state == HistoryImportState::Building
                && summary.total_issues > 0
                && summary.completed_issues == summary.total_issues
        })
        && history.pending_issue(space_id).ok().flatten().is_none()
}

async fn import_pending_history(
    state: &AppState,
    provider: &(dyn Provider + Send + Sync),
    entry: &SpaceEntry,
) {
    if state
        .history
        .summary(&entry.id)
        .ok()
        .flatten()
        .is_some_and(|summary| {
            summary.state == HistoryImportState::RateLimited
                && summary
                    .resume_at
                    .is_some_and(|resume_at| resume_at > Utc::now().timestamp())
        })
    {
        return;
    }
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
                let summary = match error {
                    ProviderError::RateLimited { reset_epoch } => state
                        .history
                        .mark_rate_limited(&entry.id, Some(rate_limit_resume(reset_epoch)))
                        .unwrap_or_else(|store_error| failed_history(store_error.to_string())),
                    other => state
                        .history
                        .mark_failed(&entry.id, other.to_string())
                        .unwrap_or_else(|store_error| failed_history(store_error.to_string())),
                };
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
                resume_cursor: page.resume_cursor,
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

fn publish_space(state: &AppState, space: SpaceModel) {
    let mut model = state.hub.borrow().clone();
    if let Some(existing) = model
        .spaces
        .iter_mut()
        .find(|existing| existing.id == space.id)
    {
        *existing = space;
    } else {
        model.spaces.push(space);
    }
    state.hub.send_replace(model);
}

fn publish_history_summary(state: &AppState, space_id: &str, summary: HistorySummary) {
    let mut model = state.hub.borrow().clone();
    let Some(space) = model.spaces.iter_mut().find(|space| space.id == space_id) else {
        return;
    };
    space.history = summary;
    state.hub.send_replace(model);
}

fn sync_result(
    entry: &SpaceEntry,
    result: Result<ProviderSnapshot, ProviderError>,
    provider: &(dyn Provider + Send + Sync),
    cache: &Cache,
    history_store: &stellr_history::HistoryStore,
) -> SpaceModel {
    match result {
        Ok(snapshot) => {
            let synced_at = Utc::now().timestamp();
            // A successful provider sync is fresh even if its fallback cache cannot be updated.
            let _ = cache.store(
                &entry.repo,
                &Snapshot {
                    viewer_login: snapshot.viewer_login.clone(),
                    issues: snapshot.issues.clone(),
                    synced_at,
                },
            );
            let history = history_after_snapshot(entry, &snapshot, history_store);
            model(
                entry,
                snapshot.issues,
                snapshot.viewer_login,
                Some(synced_at),
                false,
                None,
                history,
            )
        }
        Err(error) => cached_model(entry, provider, cache, history_store, error),
    }
}

fn cached_model(
    entry: &SpaceEntry,
    provider: &(dyn Provider + Send + Sync),
    cache: &Cache,
    history_store: &stellr_history::HistoryStore,
    error: ProviderError,
) -> SpaceModel {
    let snapshot = cache.load(&entry.repo);
    let (issues, cached_viewer_login, synced_at) = snapshot
        .map(|snapshot| {
            (
                snapshot.issues,
                snapshot.viewer_login,
                Some(snapshot.synced_at),
            )
        })
        .unwrap_or_default();
    let viewer_login = provider
        .allows_cached_viewer_identity()
        .then_some(cached_viewer_login)
        .flatten();
    let stored_history = history_store
        .summary(&entry.id)
        .unwrap_or_else(|history_error| Some(failed_history(history_error.to_string())))
        .unwrap_or_default();
    let history = match &error {
        ProviderError::RateLimited { reset_epoch }
            if stored_history.state != HistoryImportState::Unavailable =>
        {
            history_store
                .mark_rate_limited(&entry.id, Some(rate_limit_resume(*reset_epoch)))
                .unwrap_or(stored_history)
        }
        _ => stored_history,
    };
    model(
        entry,
        issues,
        viewer_login,
        synced_at,
        true,
        Some(error.to_string()),
        history,
    )
}

fn rate_limit_resume(provider_reset: Option<i64>) -> i64 {
    provider_reset.unwrap_or_else(|| {
        Utc::now()
            .timestamp()
            .saturating_add(CONSERVATIVE_RATE_LIMIT_BACKOFF)
    })
}

fn history_after_snapshot(
    entry: &SpaceEntry,
    snapshot: &stellr_core::ProviderSnapshot,
    store: &stellr_history::HistoryStore,
) -> HistorySummary {
    let Some(repository_id) = snapshot.repository_id.as_ref() else {
        return store
            .summary(&entry.id)
            .unwrap_or_else(|error| Some(failed_history(error.to_string())))
            .unwrap_or_default();
    };
    let Some(verified_through) = snapshot.history_cutoff else {
        let diagnostic = "provider snapshot omitted a history verification boundary".to_owned();
        return store
            .mark_failed(&entry.id, diagnostic.clone())
            .unwrap_or_else(|_| failed_history(diagnostic));
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
    viewer_login: Option<String>,
    synced_at: Option<i64>,
    stale: bool,
    error: Option<String>,
    history: HistorySummary,
) -> SpaceModel {
    SpaceModel {
        id: entry.id.clone(),
        repo: entry.repo.slug(),
        name: entry.repo.name.clone(),
        viewer_login,
        stars: derive(&issues),
        synced_at,
        stale,
        error,
        history,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{CONSERVATIVE_RATE_LIMIT_BACKOFF, rate_limit_resume};

    #[test]
    fn missing_provider_reset_uses_a_bounded_conservative_backoff() {
        let before = Utc::now().timestamp() + CONSERVATIVE_RATE_LIMIT_BACKOFF;
        let resume = rate_limit_resume(None);
        let after = Utc::now().timestamp() + CONSERVATIVE_RATE_LIMIT_BACKOFF;

        assert!((before..=after).contains(&resume));
    }
}
