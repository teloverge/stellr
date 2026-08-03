use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use stellr_github::device_flow::{
    DeviceFlowClient, DeviceFlowController, DeviceFlowStatus, PollOutcome,
};
use wiremock::{
    Mock, MockServer, Request, Respond, ResponseTemplate,
    matchers::{body_string_contains, header, method, path},
};

const CLIENT_ID: &str = "Ov23liWXBEZ0ysYu2MxE";

async fn mount_device_code(server: &MockServer, interval: u64) {
    Mock::given(method("POST"))
        .and(path("/login/device/code"))
        .and(header("accept", "application/json"))
        .and(body_string_contains(format!("client_id={CLIENT_ID}")))
        .and(body_string_contains("scope=repo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "device_code": "device-secret",
            "user_code": "ABCD-EFGH",
            "verification_uri": "https://github.com/login/device",
            "expires_in": 900,
            "interval": interval
        })))
        .mount(server)
        .await;
}

fn client(server: &MockServer) -> DeviceFlowClient {
    DeviceFlowClient::new(server.uri().parse().unwrap(), CLIENT_ID, "repo").unwrap()
}

async fn wait_for_calls(calls: &AtomicUsize, expected: usize) {
    for _ in 0..100 {
        if calls.load(Ordering::SeqCst) == expected {
            return;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(calls.load(Ordering::SeqCst), expected);
}

async fn settle_http_response() {
    for _ in 0..100 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test]
async fn begins_with_only_safe_operator_fields_and_the_approved_repo_scope() {
    let server = MockServer::start().await;
    mount_device_code(&server, 5).await;
    let controller = DeviceFlowController::new(client(&server));

    let status = controller.begin().await.unwrap();

    assert_eq!(
        status,
        DeviceFlowStatus::Pending {
            user_code: "ABCD-EFGH".into(),
            verification_uri: "https://github.com/login/device".into(),
            expires_in_seconds: 900,
            interval_seconds: 5,
        }
    );
    let public_json = serde_json::to_string(&status).unwrap();
    assert!(!public_json.contains("device-secret"));
    assert!(!public_json.contains("access-token"));
    controller.cancel().await;
}

#[tokio::test]
async fn maps_terminal_github_responses_without_exposing_an_access_token() {
    for (response, expected) in [
        (
            serde_json::json!({"error": "authorization_pending"}),
            PollOutcome::Pending,
        ),
        (
            serde_json::json!({"error": "slow_down"}),
            PollOutcome::SlowDown,
        ),
        (
            serde_json::json!({"error": "access_denied"}),
            PollOutcome::Denied,
        ),
        (
            serde_json::json!({"error": "expired_token"}),
            PollOutcome::Expired,
        ),
    ] {
        let server = MockServer::start().await;
        mount_device_code(&server, 0).await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .and(body_string_contains("device_code=device-secret"))
            .and(body_string_contains(
                "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .mount(&server)
            .await;
        let client = client(&server);
        let authorization = client.request_code().await.unwrap();

        assert_eq!(client.poll_once(&authorization).await.unwrap(), expected);
    }

    let server = MockServer::start().await;
    mount_device_code(&server, 0).await;
    Mock::given(method("POST"))
        .and(path("/login/oauth/access_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "access-token-must-stay-native",
            "token_type": "bearer",
            "scope": "repo"
        })))
        .mount(&server)
        .await;
    let client = client(&server);
    let authorization = client.request_code().await.unwrap();
    let outcome = client.poll_once(&authorization).await.unwrap();

    assert!(matches!(outcome, PollOutcome::Authorized(_)));
    assert!(!format!("{outcome:?}").contains("access-token-must-stay-native"));
}

#[derive(Clone)]
struct SequenceResponder {
    calls: Arc<AtomicUsize>,
}

impl Respond for SequenceResponder {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        match self.calls.fetch_add(1, Ordering::SeqCst) {
            0 => ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"error": "authorization_pending"})),
            1 => {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"error": "slow_down"}))
            }
            _ => ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "native-only-token",
                "token_type": "bearer",
                "scope": "repo"
            })),
        }
    }
}

#[tokio::test(start_paused = true)]
async fn pending_and_slow_down_respect_the_server_interval_and_add_five_seconds() {
    let server = MockServer::start().await;
    mount_device_code(&server, 1).await;
    let calls = Arc::new(AtomicUsize::new(0));
    Mock::given(method("POST"))
        .and(path("/login/oauth/access_token"))
        .respond_with(SequenceResponder {
            calls: calls.clone(),
        })
        .mount(&server)
        .await;
    let controller = DeviceFlowController::new(client(&server));
    controller.begin().await.unwrap();
    tokio::task::yield_now().await;

    tokio::time::advance(std::time::Duration::from_millis(999)).await;
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    tokio::time::advance(std::time::Duration::from_millis(1)).await;
    wait_for_calls(&calls, 1).await;
    settle_http_response().await;

    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    wait_for_calls(&calls, 2).await;
    settle_http_response().await;

    tokio::time::advance(std::time::Duration::from_millis(5_999)).await;
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    tokio::time::advance(std::time::Duration::from_millis(1)).await;
    wait_for_calls(&calls, 3).await;
    settle_http_response().await;
    assert_eq!(
        controller.status().await,
        DeviceFlowStatus::Authorized {
            storage_warning: None,
        }
    );
    assert_eq!(
        controller.take_token().await.as_deref(),
        Some("native-only-token")
    );
}

#[tokio::test(start_paused = true)]
async fn cancel_stops_polling_and_retry_starts_with_a_fresh_code() {
    let server = MockServer::start().await;
    mount_device_code(&server, 30).await;
    let calls = Arc::new(AtomicUsize::new(0));
    Mock::given(method("POST"))
        .and(path("/login/oauth/access_token"))
        .respond_with(SequenceResponder {
            calls: calls.clone(),
        })
        .mount(&server)
        .await;
    let controller = DeviceFlowController::new(client(&server));
    controller.begin().await.unwrap();

    controller.cancel().await;
    tokio::time::advance(std::time::Duration::from_secs(60)).await;
    tokio::task::yield_now().await;

    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(controller.status().await, DeviceFlowStatus::Cancelled);
    assert!(matches!(
        controller.begin().await.unwrap(),
        DeviceFlowStatus::Pending { .. }
    ));
}
