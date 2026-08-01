use std::{sync::Arc, time::Duration};

use chrono::Utc;
use stellr_core::{Model, Provider, SpaceModel, derive};
use stellr_github::cache::{Cache, Snapshot};

use crate::{spaces::SpaceEntry, state::AppState};

pub fn spawn_poller(
    state: Arc<AppState>,
    provider: Arc<dyn Provider + Send + Sync>,
    cache: Cache,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        loop {
            tokio::select! {
                _ = ticker.tick() => {}
                _ = state.refresh.notified() => {}
            }
            sync_spaces(&state, provider.as_ref(), &cache).await;
        }
    })
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
