use std::process::Stdio;

use tempfile::TempDir;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    time::{Duration, timeout},
};

async fn launch(arguments: &[&str]) -> (Child, String, TempDir) {
    let profile = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_stellr"))
        .args(arguments)
        .env("APPDATA", profile.path().join("roaming"))
        .env("LOCALAPPDATA", profile.path().join("local"))
        .env("HOME", profile.path())
        .env("XDG_CONFIG_HOME", profile.path().join("config"))
        .env("XDG_DATA_HOME", profile.path().join("data"))
        .env("GITHUB_TOKEN", "test-provider-token")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let line = timeout(
        Duration::from_secs(10),
        BufReader::new(stdout).lines().next_line(),
    )
    .await
    .expect("stellr should print its cockpit URL promptly")
    .unwrap()
    .expect("stellr should print one line before serving");
    (child, line, profile)
}

#[tokio::test]
async fn serve_without_session_auth_hosts_the_ui_and_empty_model() {
    let (mut child, line, _profile) =
        launch(&["serve", "--addr", "127.0.0.1:0", "--no-token"]).await;
    let url = line
        .strip_prefix("stellr cockpit: ")
        .expect("the startup line should use the documented prefix");

    assert!(url.starts_with("http://127.0.0.1:"));
    assert!(url.ends_with('/'));
    assert!(!url.contains('?'));

    let root = reqwest::get(url).await.unwrap();
    assert_eq!(root.status(), reqwest::StatusCode::OK);
    assert!(root.text().await.unwrap().contains("<div id=\"app\">"));

    let model = reqwest::get(format!("{url}api/model"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(model, r#"{"spaces":[]}"#);

    child.kill().await.unwrap();
}

#[tokio::test]
async fn serve_generates_a_session_token_and_gates_the_ui_by_default() {
    let (mut child, line, _profile) = launch(&["serve", "--addr", "127.0.0.1:0"]).await;
    let url = line
        .strip_prefix("stellr cockpit: ")
        .expect("the startup line should use the documented prefix");
    let tokened_url = reqwest::Url::parse(url).unwrap();
    let token = tokened_url
        .query_pairs()
        .find_map(|(name, value)| (name == "token").then(|| value.into_owned()))
        .expect("the default cockpit URL should carry a session token");

    assert_eq!(token.len(), 32);
    assert!(
        token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );

    let mut bare_url = tokened_url.clone();
    bare_url.set_query(None);
    assert_eq!(
        reqwest::get(bare_url).await.unwrap().status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        reqwest::get(tokened_url).await.unwrap().status(),
        reqwest::StatusCode::OK
    );

    child.kill().await.unwrap();
}
