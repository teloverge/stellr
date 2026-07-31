use std::sync::Arc;

use stellr_core::Model;
use stellr_server::state::AppState;

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
