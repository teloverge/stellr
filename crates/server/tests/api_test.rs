use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use stellr_core::{
    IssueState, Model, Provider, ProviderError, ProviderSnapshot, RawIssue, RepoRef, SpaceModel,
};
use stellr_github::cache::{Cache, Snapshot};
use stellr_server::{
    poll::spawn_poller,
    spaces::{SpaceEntry, SpaceStore},
    state::AppState,
};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

async fn serve(state: Arc<AppState>) -> String {
    let app = stellr_server::routes::router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

async fn serve_until_shutdown(state: Arc<AppState>) -> (String, tokio::sync::oneshot::Sender<()>) {
    let app = stellr_server::routes::router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (shutdown, shutdown_signal) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_signal.await;
            })
            .await
            .unwrap()
    });
    (format!("http://{addr}"), shutdown)
}

fn state(token: Option<&str>) -> Arc<AppState> {
    let (hub, _receiver) = tokio::sync::watch::channel(Model { spaces: vec![] });
    state_with_hub(hub, token)
}

fn state_with_hub(hub: tokio::sync::watch::Sender<Model>, token: Option<&str>) -> Arc<AppState> {
    Arc::new(AppState {
        hub,
        token: token.map(str::to_owned),
        spaces: tokio::sync::Mutex::new(SpaceStore::load(std::path::PathBuf::new())),
        refresh: Arc::new(tokio::sync::Notify::new()),
    })
}

fn control_url(base: &str) -> String {
    base.replacen("http://", "ws://", 1) + "/ws/control"
}

fn model_with_space(id: &str) -> Model {
    Model {
        spaces: vec![SpaceModel {
            id: id.into(),
            repo: "owner/repo".into(),
            name: "repo".into(),
            viewer_login: None,
            stars: vec![],
            synced_at: None,
            stale: false,
            error: None,
        }],
    }
}

#[tokio::test]
async fn embedded_ui_serves_the_built_index_at_the_root() {
    let base = serve(state(None)).await;

    let response = reqwest::get(format!("{base}/")).await.unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .unwrap(),
        "text/html"
    );
    assert!(response.text().await.unwrap().contains("<div id=\"app\">"));
}

#[tokio::test]
async fn embedded_ui_assets_remain_public_when_api_requires_authentication() {
    let base = serve(state(Some("session-token"))).await;
    let client = reqwest::Client::new();

    let index = client.get(format!("{base}/")).send().await.unwrap();
    assert_eq!(index.status(), reqwest::StatusCode::OK);
    let html = index.text().await.unwrap();
    let asset_path = html
        .split("src=\"")
        .nth(1)
        .and_then(|tail| tail.split('"').next())
        .expect("the embedded index should reference its built script");
    assert!(asset_path.starts_with("/assets/"));

    let asset = client
        .get(format!("{base}{asset_path}"))
        .send()
        .await
        .unwrap();
    assert_eq!(asset.status(), reqwest::StatusCode::OK);
}

#[tokio::test]
async fn embedded_ui_uses_the_index_for_spa_paths() {
    let base = serve(state(None)).await;

    let response = reqwest::get(format!("{base}/spaces/o-r")).await.unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .unwrap(),
        "text/html"
    );
    assert!(response.text().await.unwrap().contains("<div id=\"app\">"));
}

#[tokio::test]
async fn embedded_ui_does_not_mask_unknown_api_paths() {
    let base = serve(state(None)).await;

    let response = reqwest::get(format!("{base}/api/unknown")).await.unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
}

struct StubProvider(ProviderSnapshot);

#[async_trait::async_trait]
impl Provider for StubProvider {
    async fn fetch(&self, _repo: &RepoRef) -> Result<ProviderSnapshot, ProviderError> {
        Ok(self.0.clone())
    }
}

struct FailingProvider;

#[async_trait::async_trait]
impl Provider for FailingProvider {
    async fn fetch(&self, _repo: &RepoRef) -> Result<ProviderSnapshot, ProviderError> {
        Err(ProviderError::Http("offline".into()))
    }
}

struct SequenceProvider(AtomicUsize);

#[async_trait::async_trait]
impl Provider for SequenceProvider {
    async fn fetch(&self, _repo: &RepoRef) -> Result<ProviderSnapshot, ProviderError> {
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            return Ok(ProviderSnapshot::without_viewer(vec![]));
        }
        Ok(ProviderSnapshot::without_viewer(vec![RawIssue {
            number: 9,
            parent_issue: None,
            title: "Arrived on the second tick".into(),
            body: String::new(),
            state: IssueState::Open,
            assignees: vec![],
            milestone: None,
            labels: vec![],
            blocked_by: vec![],
            url: "https://github.com/o/r/issues/9".into(),
        }]))
    }
}

#[tokio::test]
async fn add_repo_space_immediately_populates_the_model() {
    let directory = tempfile::tempdir().unwrap();
    let (hub, mut receiver) = tokio::sync::watch::channel(Model { spaces: vec![] });
    let state = Arc::new(AppState {
        hub,
        token: None,
        spaces: tokio::sync::Mutex::new(SpaceStore::load(directory.path().join("spaces.toml"))),
        refresh: Arc::new(tokio::sync::Notify::new()),
    });
    let poller = spawn_poller(
        state.clone(),
        Arc::new(StubProvider(ProviderSnapshot {
            viewer_login: Some("octocat".into()),
            issues: vec![RawIssue {
                number: 1,
                parent_issue: None,
                title: "Ready work".into(),
                body: String::new(),
                state: IssueState::Open,
                assignees: vec!["OctoCat".into()],
                milestone: None,
                labels: vec!["ready-for-agent".into()],
                blocked_by: vec![],
                url: "https://github.com/o/r/issues/1".into(),
            }],
        })),
        Cache::new(directory.path().join("cache")),
        Duration::from_secs(60),
    );
    tokio::time::timeout(Duration::from_secs(1), receiver.changed())
        .await
        .expect("the startup poll should complete")
        .expect("the model hub should remain open");
    let base = serve(state).await;
    let client = reqwest::Client::new();

    let added = client
        .post(format!("{base}/api/spaces"))
        .json(&json!({ "repo": "o/r" }))
        .send()
        .await
        .unwrap();
    assert_eq!(added.status(), reqwest::StatusCode::OK);
    assert_eq!(
        added.json::<serde_json::Value>().await.unwrap(),
        json!({ "id": "o-r" })
    );

    let model = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let model = client
                .get(format!("{base}/api/model"))
                .send()
                .await
                .unwrap()
                .json::<Model>()
                .await
                .unwrap();
            if model
                .spaces
                .first()
                .is_some_and(|space| space.stars.len() == 1)
            {
                break model;
            }
        }
    })
    .await
    .expect("adding a space should publish the derived model");

    assert_eq!(model.spaces[0].id, "o-r");
    assert_eq!(model.spaces[0].repo, "o/r");
    assert_eq!(model.spaces[0].viewer_login.as_deref(), Some("octocat"));
    assert_eq!(model.spaces[0].stars[0].number, 1);
    assert!(model.spaces[0].stars[0].ready_for_agent);
    poller.abort();
}

#[tokio::test]
async fn failed_sync_publishes_the_cached_model_as_stale_with_the_error() {
    let directory = tempfile::tempdir().unwrap();
    let repo = RepoRef {
        owner: "o".into(),
        name: "r".into(),
    };
    let cache = Cache::new(directory.path().join("cache"));
    cache
        .store(
            &repo,
            &Snapshot {
                issues: vec![RawIssue {
                    number: 7,
                    parent_issue: None,
                    title: "Cached work".into(),
                    body: String::new(),
                    state: IssueState::Open,
                    assignees: vec![],
                    milestone: None,
                    labels: vec![],
                    blocked_by: vec![],
                    url: "https://github.com/o/r/issues/7".into(),
                }],
                synced_at: 1_753_000_000,
            },
        )
        .unwrap();
    let mut spaces = SpaceStore::load(directory.path().join("spaces.toml"));
    spaces.add(SpaceEntry::new(repo, None)).unwrap();
    spaces.save().unwrap();
    let (hub, _receiver) = tokio::sync::watch::channel(Model { spaces: vec![] });
    let state = Arc::new(AppState {
        hub,
        token: None,
        spaces: tokio::sync::Mutex::new(spaces),
        refresh: Arc::new(tokio::sync::Notify::new()),
    });
    let poller = spawn_poller(
        state.clone(),
        Arc::new(FailingProvider),
        cache,
        Duration::from_secs(60),
    );
    let base = serve(state).await;
    let client = reqwest::Client::new();

    let model = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let model = client
                .get(format!("{base}/api/model"))
                .send()
                .await
                .unwrap()
                .json::<Model>()
                .await
                .unwrap();
            if !model.spaces.is_empty() {
                break model;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("failed sync should still publish the cached model");

    assert_eq!(model.spaces[0].stars[0].number, 7);
    assert_eq!(model.spaces[0].synced_at, Some(1_753_000_000));
    assert!(model.spaces[0].stale);
    assert_eq!(
        model.spaces[0].error.as_deref(),
        Some("HTTP request failed: offline")
    );
    poller.abort();
}

#[tokio::test]
async fn successful_sync_stays_fresh_when_the_cache_cannot_be_written() {
    let directory = tempfile::tempdir().unwrap();
    let cache_root = directory.path().join("cache");
    std::fs::File::create(&cache_root).unwrap();
    let mut spaces = SpaceStore::load(directory.path().join("spaces.toml"));
    spaces
        .add(SpaceEntry::new(
            RepoRef {
                owner: "o".into(),
                name: "r".into(),
            },
            None,
        ))
        .unwrap();
    let (hub, mut receiver) = tokio::sync::watch::channel(Model { spaces: vec![] });
    let state = Arc::new(AppState {
        hub,
        token: None,
        spaces: tokio::sync::Mutex::new(spaces),
        refresh: Arc::new(tokio::sync::Notify::new()),
    });
    let poller = spawn_poller(
        state,
        Arc::new(StubProvider(ProviderSnapshot::without_viewer(vec![]))),
        Cache::new(cache_root),
        Duration::from_secs(60),
    );

    tokio::time::timeout(Duration::from_secs(1), receiver.changed())
        .await
        .expect("the startup poll should complete")
        .expect("the model hub should remain open");
    let model = receiver.borrow_and_update().clone();

    assert!(!model.spaces[0].stale);
    assert_eq!(model.spaces[0].error, None);
    poller.abort();
}

#[tokio::test]
async fn delete_space_removes_the_persisted_entry() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("spaces.toml");
    let (hub, _receiver) = tokio::sync::watch::channel(Model { spaces: vec![] });
    let state = Arc::new(AppState {
        hub,
        token: None,
        spaces: tokio::sync::Mutex::new(SpaceStore::load(file.clone())),
        refresh: Arc::new(tokio::sync::Notify::new()),
    });
    let base = serve(state).await;
    let client = reqwest::Client::new();
    assert_eq!(
        client
            .post(format!("{base}/api/spaces"))
            .json(&json!({ "repo": "o/r" }))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::OK
    );

    let deleted = client
        .delete(format!("{base}/api/spaces/o-r"))
        .send()
        .await
        .unwrap();

    assert_eq!(deleted.status(), reqwest::StatusCode::NO_CONTENT);
    assert!(SpaceStore::load(file).entries().is_empty());
}

#[tokio::test]
async fn add_path_space_detects_and_persists_the_github_origin() {
    let directory = tempfile::tempdir().unwrap();
    let repo_path = directory.path().join("repo");
    std::fs::create_dir(&repo_path).unwrap();
    assert!(
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&repo_path)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        std::process::Command::new("git")
            .args([
                "remote",
                "add",
                "origin",
                "git@github.com:owner/repository.git",
            ])
            .current_dir(&repo_path)
            .status()
            .unwrap()
            .success()
    );
    let file = directory.path().join("spaces.toml");
    let (hub, _receiver) = tokio::sync::watch::channel(Model { spaces: vec![] });
    let state = Arc::new(AppState {
        hub,
        token: None,
        spaces: tokio::sync::Mutex::new(SpaceStore::load(file.clone())),
        refresh: Arc::new(tokio::sync::Notify::new()),
    });
    let base = serve(state).await;

    let response = reqwest::Client::new()
        .post(format!("{base}/api/spaces"))
        .json(&json!({ "path": repo_path }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        json!({ "id": "owner-repository" })
    );
    let reloaded = SpaceStore::load(file);
    assert_eq!(reloaded.entries().len(), 1);
    assert_eq!(reloaded.entries()[0].repo.slug(), "owner/repository");
    assert_eq!(
        reloaded.entries()[0].path.as_deref(),
        Some(repo_path.as_path())
    );
}

#[tokio::test]
async fn poller_repeats_on_the_configured_interval() {
    let directory = tempfile::tempdir().unwrap();
    let mut spaces = SpaceStore::load(directory.path().join("spaces.toml"));
    spaces
        .add(SpaceEntry::new(
            RepoRef {
                owner: "o".into(),
                name: "r".into(),
            },
            None,
        ))
        .unwrap();
    let (hub, mut receiver) = tokio::sync::watch::channel(Model { spaces: vec![] });
    let state = Arc::new(AppState {
        hub,
        token: None,
        spaces: tokio::sync::Mutex::new(spaces),
        refresh: Arc::new(tokio::sync::Notify::new()),
    });
    let poller = spawn_poller(
        state.clone(),
        Arc::new(SequenceProvider(AtomicUsize::new(0))),
        Cache::new(directory.path().join("cache")),
        Duration::from_millis(50),
    );

    let model = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            receiver.changed().await.unwrap();
            let model = receiver.borrow_and_update().clone();
            if model
                .spaces
                .first()
                .is_some_and(|space| !space.stars.is_empty())
            {
                break model;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the second interval tick should publish the new issue");

    assert_eq!(model.spaces[0].stars[0].number, 9);
    poller.abort();
}

#[tokio::test]
async fn control_socket_sends_current_snapshot_on_connect() {
    let base = serve(state(None)).await;
    let ws_url = control_url(&base);

    let (mut socket, _) = tokio_tungstenite::connect_async(ws_url).await.unwrap();
    let frame = tokio::time::timeout(std::time::Duration::from_secs(1), socket.next())
        .await
        .expect("snapshot should arrive promptly")
        .expect("socket should remain open")
        .expect("snapshot should be a valid WebSocket frame")
        .into_text()
        .expect("snapshot should be a text frame");

    let model: Model = serde_json::from_str(&frame).unwrap();
    assert!(model.spaces.is_empty());
}

#[tokio::test]
async fn control_socket_reconnect_starts_with_latest_non_empty_snapshot() {
    let (hub, _receiver) = tokio::sync::watch::channel(model_with_space("before-first-connect"));
    let base = serve(state_with_hub(hub.clone(), None)).await;
    let ws_url = control_url(&base);

    let (mut first_socket, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();
    let first_frame = tokio::time::timeout(std::time::Duration::from_secs(1), first_socket.next())
        .await
        .expect("current snapshot should arrive promptly")
        .expect("socket should remain open")
        .expect("current snapshot should be a valid WebSocket frame")
        .into_text()
        .expect("current snapshot should be a text frame");
    let first_model: Model = serde_json::from_str(&first_frame).unwrap();
    assert_eq!(first_model, model_with_space("before-first-connect"));

    first_socket.close(None).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), first_socket.next())
        .await
        .expect("server should finish the close handshake promptly");

    hub.send_replace(model_with_space("latest-before-reconnect"));
    let (mut reconnected, _) = tokio_tungstenite::connect_async(ws_url).await.unwrap();
    let reconnect_frame =
        tokio::time::timeout(std::time::Duration::from_secs(1), reconnected.next())
            .await
            .expect("latest snapshot should be the first reconnect frame")
            .expect("reconnected socket should remain open")
            .expect("latest snapshot should be a valid WebSocket frame")
            .into_text()
            .expect("latest snapshot should be a text frame");
    let reconnect_model: Model = serde_json::from_str(&reconnect_frame).unwrap();
    assert_eq!(reconnect_model, model_with_space("latest-before-reconnect"));
}

#[tokio::test]
async fn control_socket_sends_fresh_snapshot_on_model_change() {
    let (hub, _receiver) = tokio::sync::watch::channel(Model { spaces: vec![] });
    let base = serve(state_with_hub(hub.clone(), None)).await;
    let ws_url = control_url(&base);
    let (mut socket, _) = tokio_tungstenite::connect_async(ws_url).await.unwrap();

    socket.next().await.unwrap().unwrap();
    hub.send_replace(model_with_space("space-1"));

    let frame = tokio::time::timeout(std::time::Duration::from_secs(1), socket.next())
        .await
        .expect("changed snapshot should arrive promptly")
        .expect("socket should remain open")
        .expect("changed snapshot should be a valid WebSocket frame")
        .into_text()
        .expect("changed snapshot should be a text frame");
    let model: Model = serde_json::from_str(&frame).unwrap();
    assert_eq!(model, model_with_space("space-1"));
}

#[tokio::test]
async fn control_socket_closes_quietly_when_watch_channel_closes() {
    let (hub, _receiver) = tokio::sync::watch::channel(Model { spaces: vec![] });
    let (base, shutdown) = serve_until_shutdown(state_with_hub(hub.clone(), None)).await;
    let ws_url = control_url(&base);
    let (mut socket, _) = tokio_tungstenite::connect_async(ws_url).await.unwrap();

    socket.next().await.unwrap().unwrap();
    drop(hub);
    shutdown.send(()).unwrap();

    let closure = tokio::time::timeout(std::time::Duration::from_secs(1), socket.next())
        .await
        .expect("closed watch channel should end the active socket promptly");
    assert!(
        matches!(
            closure,
            None | Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_)))
        ),
        "unexpected terminal websocket result: {closure:?}"
    );
}

#[tokio::test]
async fn control_socket_ignores_client_data_and_keeps_streaming_snapshots() {
    let (hub, _receiver) = tokio::sync::watch::channel(Model { spaces: vec![] });
    let base = serve(state_with_hub(hub.clone(), None)).await;
    let ws_url = control_url(&base);
    let (mut socket, _) = tokio_tungstenite::connect_async(ws_url).await.unwrap();

    socket.next().await.unwrap().unwrap();
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            "not-a-command".into(),
        ))
        .await
        .unwrap();
    hub.send_replace(model_with_space("still-streaming"));

    let frame = tokio::time::timeout(std::time::Duration::from_secs(1), socket.next())
        .await
        .expect("changed snapshot should arrive after irrelevant client data")
        .expect("socket should remain open")
        .expect("changed snapshot should be a valid WebSocket frame")
        .into_text()
        .expect("changed snapshot should be a text frame");
    let model: Model = serde_json::from_str(&frame).unwrap();
    assert_eq!(model, model_with_space("still-streaming"));
}

#[tokio::test]
async fn control_socket_closes_quietly_when_client_departs() {
    let base = serve(state(None)).await;
    let ws_url = control_url(&base);
    let (mut socket, _) = tokio_tungstenite::connect_async(ws_url).await.unwrap();

    socket.next().await.unwrap().unwrap();
    socket.close(None).await.unwrap();

    let closure = tokio::time::timeout(std::time::Duration::from_secs(1), socket.next())
        .await
        .expect("server should finish the close handshake promptly");
    assert!(matches!(
        closure,
        None | Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_)))
    ));
}

#[tokio::test]
async fn protected_control_socket_rejects_missing_or_inexact_tokens() {
    let base = serve(state(Some("session-token"))).await;
    let ws_url = control_url(&base);

    for denied_url in [&ws_url, &format!("{ws_url}?token=session-token-extra")] {
        let error = tokio_tungstenite::connect_async(denied_url)
            .await
            .expect_err("protected socket handshake should fail closed");
        let tokio_tungstenite::tungstenite::Error::Http(response) = error else {
            panic!("expected an HTTP handshake rejection, got {error:?}");
        };
        assert_eq!(response.status().as_u16(), 401);
        assert!(response.body().as_ref().is_none_or(Vec::is_empty));
    }
}

#[tokio::test]
async fn protected_control_socket_accepts_exact_query_or_cookie_token() {
    let base = serve(state(Some("session-token"))).await;
    let ws_url = control_url(&base);

    let query_request = format!("{ws_url}?token=session-token")
        .into_client_request()
        .unwrap();
    let mut cookie_request = ws_url.into_client_request().unwrap();
    cookie_request.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::COOKIE,
        "stellr_token=session-token".parse().unwrap(),
    );

    for request in [query_request, cookie_request] {
        let (mut socket, response) = tokio_tungstenite::connect_async(request).await.unwrap();
        assert_eq!(response.status().as_u16(), 101);

        let frame = socket
            .next()
            .await
            .expect("authenticated socket should remain open")
            .expect("snapshot should be a valid WebSocket frame")
            .into_text()
            .expect("snapshot should be a text frame");
        assert_eq!(frame, r#"{"spaces":[]}"#);

        socket.close(None).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), socket.next())
            .await
            .expect("server should finish the close handshake promptly");
    }
}

#[tokio::test]
async fn model_endpoint_serves_current_snapshot_when_open() {
    let base = serve(state(None)).await;

    let response = reqwest::get(format!("{base}/api/model")).await.unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let model: Model = response.json().await.unwrap();
    assert!(model.spaces.is_empty());
}

#[tokio::test]
async fn token_query_exchanges_for_a_strict_http_only_session_cookie() {
    let base = serve(state(Some("sekrit"))).await;
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();

    assert_eq!(
        client
            .get(format!("{base}/api/model"))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );

    let exchange = client
        .get(format!("{base}/api/model?token=sekrit"))
        .send()
        .await
        .unwrap();
    assert_eq!(exchange.status(), reqwest::StatusCode::OK);
    let set_cookie = exchange.headers().get(reqwest::header::SET_COOKIE).unwrap();
    assert_eq!(
        set_cookie.to_str().unwrap(),
        "stellr_token=sekrit; HttpOnly; SameSite=Strict; Path=/"
    );

    assert_eq!(
        client
            .get(format!("{base}/api/model"))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::OK
    );
}

#[tokio::test]
async fn exact_cookie_and_bearer_tokens_are_accepted_without_setting_a_cookie() {
    let base = serve(state(Some("sekrit"))).await;
    let client = reqwest::Client::new();

    let cookie = client
        .get(format!("{base}/api/model"))
        .header(reqwest::header::COOKIE, "other=value; stellr_token=sekrit")
        .send()
        .await
        .unwrap();
    assert_eq!(cookie.status(), reqwest::StatusCode::OK);
    assert!(cookie.headers().get(reqwest::header::SET_COOKIE).is_none());

    let bearer = client
        .get(format!("{base}/api/model"))
        .bearer_auth("sekrit")
        .send()
        .await
        .unwrap();
    assert_eq!(bearer.status(), reqwest::StatusCode::OK);
    assert!(bearer.headers().get(reqwest::header::SET_COOKIE).is_none());
}

#[tokio::test]
async fn token_auth_rejects_partial_or_wrong_values() {
    let base = serve(state(Some("sekrit"))).await;
    let client = reqwest::Client::new();

    for request in [
        client.get(format!("{base}/api/model?token=sekrit-extra")),
        client
            .get(format!("{base}/api/model"))
            .header(reqwest::header::COOKIE, "stellr_token=sekrit-extra"),
        client
            .get(format!("{base}/api/model"))
            .header(reqwest::header::AUTHORIZATION, "Bearer sekrit-extra"),
    ] {
        assert_eq!(
            request.send().await.unwrap().status(),
            reqwest::StatusCode::UNAUTHORIZED
        );
    }
}
