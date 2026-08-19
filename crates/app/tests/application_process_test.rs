#![cfg(feature = "desktop")]

use std::process::Stdio;

use axum::{Json, Router, routing::post};
use serde_json::json;
use tokio::process::Command;

async fn controlled_github() -> (String, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route(
            "/login/device/code",
            post(|| async {
                Json(json!({
                    "device_code": "controlled-device-secret",
                    "user_code": "ABCD-EFGH",
                    "verification_uri": "https://github.com/login/device",
                    "expires_in": 900,
                    "interval": 0
                }))
            }),
        )
        .route(
            "/login/oauth/access_token",
            post(|| async {
                Json(json!({
                    "access_token": "controlled-native-token",
                    "token_type": "bearer",
                    "scope": "repo"
                }))
            }),
        )
        .route(
            "/graphql",
            post(|| async {
                Json(json!({
                    "data": {
                        "viewer": { "login": "octocat" },
                        "repository": {
                            "issues": {
                                "pageInfo": { "hasNextPage": false, "endCursor": null },
                                "nodes": [{
                                    "number": 70,
                                    "title": "M2 controlled process evidence",
                                    "body": "",
                                    "url": "https://github.com/teloverge/stellr/issues/70",
                                    "state": "OPEN",
                                    "stateReason": null,
                                    "assignees": { "nodes": [] },
                                    "milestone": null,
                                    "labels": { "nodes": [] },
                                    "parent": null,
                                    "blockedBy": { "nodes": [] }
                                }]
                            }
                        }
                    }
                }))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{address}/"), server)
}

#[tokio::test]
async fn real_binary_completes_controlled_authorization_sync_http_and_focus_cadence() {
    let (github_base, server) = controlled_github().await;
    let profile = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_stellr"))
        .args([
            "acceptance",
            "--github-base",
            &github_base,
            "--profile",
            profile.path().to_str().unwrap(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .unwrap();
    server.abort();

    assert!(
        output.status.success(),
        "acceptance process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    for marker in [
        "APPLICATION_PROCESS_DEVICE_FLOW_PASSED=true",
        "APPLICATION_PROCESS_CREDENTIAL_STORE_PASSED=true",
        "APPLICATION_PROCESS_SYNC_PASSED=true",
        "APPLICATION_PROCESS_HTTP_CONTRACT_PASSED=true",
        "APPLICATION_PROCESS_FOCUS_CADENCE=30,300,30",
    ] {
        assert!(stdout.contains(marker), "missing marker: {marker}");
    }
    assert!(!stdout.contains("controlled-native-token"));
}
