use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use stellr_core::{Model, SpaceModel};
use stellr_server::state::AppState;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

async fn serve(state: Arc<AppState>) -> String {
    let app = stellr_server::routes::router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

fn state(token: Option<&str>) -> Arc<AppState> {
    let (hub, _receiver) = tokio::sync::watch::channel(Model { spaces: vec![] });
    Arc::new(AppState {
        hub,
        token: token.map(str::to_owned),
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
            stars: vec![],
            synced_at: None,
            stale: false,
            error: None,
        }],
    }
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
    let base = serve(Arc::new(AppState {
        hub: hub.clone(),
        token: None,
    }))
    .await;
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
    let base = serve(Arc::new(AppState {
        hub: hub.clone(),
        token: None,
    }))
    .await;
    let ws_url = control_url(&base);
    let (mut socket, _) = tokio_tungstenite::connect_async(ws_url).await.unwrap();

    socket.next().await.unwrap().unwrap();
    hub.send_replace(Model {
        spaces: vec![SpaceModel {
            id: "space-1".into(),
            repo: "owner/repo".into(),
            name: "repo".into(),
            stars: vec![],
            synced_at: None,
            stale: false,
            error: None,
        }],
    });

    let frame = tokio::time::timeout(std::time::Duration::from_secs(1), socket.next())
        .await
        .expect("changed snapshot should arrive promptly")
        .expect("socket should remain open")
        .expect("changed snapshot should be a valid WebSocket frame")
        .into_text()
        .expect("changed snapshot should be a text frame");
    let model: Model = serde_json::from_str(&frame).unwrap();
    assert_eq!(model.spaces.len(), 1);
    assert_eq!(model.spaces[0].id, "space-1");
}

#[tokio::test]
async fn control_socket_ignores_client_data_and_keeps_streaming_snapshots() {
    let (hub, _receiver) = tokio::sync::watch::channel(Model { spaces: vec![] });
    let base = serve(Arc::new(AppState {
        hub: hub.clone(),
        token: None,
    }))
    .await;
    let ws_url = control_url(&base);
    let (mut socket, _) = tokio_tungstenite::connect_async(ws_url).await.unwrap();

    socket.next().await.unwrap().unwrap();
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            "not-a-command".into(),
        ))
        .await
        .unwrap();
    hub.send_replace(Model {
        spaces: vec![SpaceModel {
            id: "still-streaming".into(),
            repo: "owner/repo".into(),
            name: "repo".into(),
            stars: vec![],
            synced_at: None,
            stale: false,
            error: None,
        }],
    });

    let frame = tokio::time::timeout(std::time::Duration::from_secs(1), socket.next())
        .await
        .expect("changed snapshot should arrive after irrelevant client data")
        .expect("socket should remain open")
        .expect("changed snapshot should be a valid WebSocket frame")
        .into_text()
        .expect("changed snapshot should be a text frame");
    let model: Model = serde_json::from_str(&frame).unwrap();
    assert_eq!(model.spaces[0].id, "still-streaming");
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
