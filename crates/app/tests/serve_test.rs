use std::process::Stdio;

use tempfile::TempDir;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    time::{Duration, timeout},
};

async fn launch_with_session_token(
    arguments: &[&str],
    session_token: Option<&str>,
) -> (Child, String, TempDir) {
    let profile = tempfile::tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_stellr"));
    command
        .args(arguments)
        .env("APPDATA", profile.path().join("roaming"))
        .env("LOCALAPPDATA", profile.path().join("local"))
        .env("HOME", profile.path())
        .env("XDG_CONFIG_HOME", profile.path().join("config"))
        .env("XDG_DATA_HOME", profile.path().join("data"))
        .env("GITHUB_TOKEN", "test-provider-token")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(session_token) = session_token {
        command.env("STELLR_SESSION_TOKEN", session_token);
    }
    let mut child = command.spawn().unwrap();
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

async fn launch(arguments: &[&str]) -> (Child, String, TempDir) {
    launch_with_session_token(arguments, None).await
}

#[tokio::test]
async fn serve_without_session_auth_hosts_the_ui_and_empty_model() {
    let (mut child, line, _profile) = launch(&[
        "serve",
        "--addr",
        "127.0.0.1:0",
        "--no-token",
        "--issue",
        "14",
    ])
    .await;
    let raw_url = line
        .strip_prefix("stellr cockpit: ")
        .expect("the startup line should use the documented prefix");
    let url = reqwest::Url::parse(raw_url).unwrap();

    assert!(raw_url.starts_with("http://127.0.0.1:"));
    assert_eq!(
        url.query_pairs().collect::<Vec<_>>(),
        vec![("issue".into(), "14".into())]
    );

    let root = reqwest::get(url.clone()).await.unwrap();
    assert_eq!(root.status(), reqwest::StatusCode::OK);
    assert!(root.text().await.unwrap().contains("<div id=\"app\">"));

    let model = reqwest::get(url.join("api/model").unwrap())
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(model, r#"{"spaces":[]}"#);

    child.kill().await.unwrap();
}

#[tokio::test]
async fn serve_generates_a_session_token_and_gates_protected_routes_by_default() {
    let (mut child, line, _profile) =
        launch(&["serve", "--addr", "127.0.0.1:0", "--issue", "14"]).await;
    let url = line
        .strip_prefix("stellr cockpit: ")
        .expect("the startup line should use the documented prefix");
    let tokened_url = reqwest::Url::parse(url).unwrap();
    let token = tokened_url
        .query_pairs()
        .find_map(|(name, value)| (name == "token").then(|| value.into_owned()))
        .expect("the default cockpit URL should carry a session token");
    let issue = tokened_url
        .query_pairs()
        .find_map(|(name, value)| (name == "issue").then(|| value.into_owned()));

    assert_eq!(token.len(), 32);
    assert!(
        token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );

    let mut bare_url = tokened_url.clone();
    bare_url.set_query(None);
    assert_eq!(
        reqwest::get(bare_url.clone()).await.unwrap().status(),
        reqwest::StatusCode::OK
    );
    assert_eq!(issue.as_deref(), Some("14"));

    let mut protected_url = bare_url.join("api/model").unwrap();
    assert_eq!(
        reqwest::get(protected_url.clone()).await.unwrap().status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    protected_url.set_query(tokened_url.query());
    assert_eq!(
        reqwest::get(protected_url).await.unwrap().status(),
        reqwest::StatusCode::OK
    );

    child.kill().await.unwrap();
}

#[tokio::test]
async fn serve_reuses_a_configured_session_token() {
    let expected = "0123456789abcdef0123456789abcdef";
    let (mut child, line, _profile) =
        launch_with_session_token(&["serve", "--addr", "127.0.0.1:0"], Some(expected)).await;
    let url = line
        .strip_prefix("stellr cockpit: ")
        .expect("the startup line should use the documented prefix");
    let token = reqwest::Url::parse(url)
        .unwrap()
        .query_pairs()
        .find_map(|(name, value)| (name == "token").then(|| value.into_owned()));

    assert_eq!(token.as_deref(), Some(expected));
    child.kill().await.unwrap();
}
