use std::{sync::Arc, time::Duration};

use chrono::Utc;
use stellr_core::{Model, Provider, SpaceModel, derive};
use stellr_github::cache::{Cache, Snapshot};

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
    for entry in entries {
        spaces.push(sync_space(&entry, provider, cache).await);
    }
    state.hub.send_replace(Model { spaces });
}

async fn sync_space(
    entry: &SpaceEntry,
    provider: &(dyn Provider + Send + Sync),
    cache: &Cache,
) -> SpaceModel {
    match provider.fetch(&entry.repo).await {
        Ok(issues) => {
            let synced_at = Utc::now().timestamp();
            // A successful provider sync is fresh even if its fallback cache cannot be updated.
            let _ = cache.store(
                &entry.repo,
                &Snapshot {
                    issues: issues.clone(),
                    synced_at,
                },
            );
            model(entry, issues, Some(synced_at), false, None)
        }
        Err(error) => {
            let snapshot = cache.load(&entry.repo);
            let (issues, synced_at) = snapshot
                .map(|snapshot| (snapshot.issues, Some(snapshot.synced_at)))
                .unwrap_or_default();
            model(entry, issues, synced_at, true, Some(error.to_string()))
        }
    }
}

fn model(
    entry: &SpaceEntry,
    issues: Vec<stellr_core::RawIssue>,
    synced_at: Option<i64>,
    stale: bool,
    error: Option<String>,
) -> SpaceModel {
    SpaceModel {
        id: entry.id.clone(),
        repo: entry.repo.slug(),
        name: entry.repo.name.clone(),
        stars: derive(&issues),
        synced_at,
        stale,
        error,
    }
}
