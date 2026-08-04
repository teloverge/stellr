//! Debug-only deterministic application-process acceptance scenario.

use std::{path::PathBuf, sync::Arc};

use stellr_core::{Provider, ProviderError, RawIssue, RepoRef};
use stellr_github::{
    credentials::{CredentialStore, CredentialStoreError},
    device_flow::{AccessToken, DeviceFlowClient, DeviceFlowController, DeviceFlowStatus},
    sync::GithubProvider,
};
use stellr_server::{poll::PollingControl, spaces::SpaceEntry};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    auth_activation::activate_provider_and_store,
    desktop::{
        BACKGROUND_POLL_INTERVAL, DesktopRuntimeOptions, FOCUSED_POLL_INTERVAL,
        start_runtime_with_entry,
    },
    runtime::ProviderSlot,
};

type DynError = Box<dyn std::error::Error + Send + Sync>;

struct SignedOut;

#[async_trait::async_trait]
impl Provider for SignedOut {
    async fn fetch(&self, _repo: &RepoRef) -> Result<Vec<RawIssue>, ProviderError> {
        Err(ProviderError::Auth("GitHub sign-in required".into()))
    }
}

struct TestCredentialStore {
    path: PathBuf,
}

impl CredentialStore for TestCredentialStore {
    fn load(&self) -> Result<Option<String>, CredentialStoreError> {
        match std::fs::read_to_string(&self.path) {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(CredentialStoreError::Message(error.to_string())),
        }
    }

    fn store(&self, credential: &str) -> Result<(), CredentialStoreError> {
        if credential.trim().is_empty() {
            return Err(CredentialStoreError::Message(
                "refusing to store a blank test credential".into(),
            ));
        }
        std::fs::write(&self.path, credential)
            .map_err(|error| CredentialStoreError::Message(error.to_string()))
    }
}

async fn wait_for_authorization(
    controller: &DeviceFlowController,
) -> Result<AccessToken, DynError> {
    controller.begin().await?;
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            match controller.status().await {
                DeviceFlowStatus::Authorized { .. } => {
                    return controller
                        .take_token()
                        .await
                        .ok_or_else(|| "authorized device flow omitted its native token".into());
                }
                DeviceFlowStatus::Denied
                | DeviceFlowStatus::Expired
                | DeviceFlowStatus::Cancelled
                | DeviceFlowStatus::Failed { .. } => {
                    return Err("controlled device flow ended without authorization".into());
                }
                _ => tokio::task::yield_now().await,
            }
        }
    })
    .await?
}

async fn wait_for_issue(runtime: &crate::runtime::ApplicationRuntime) -> Result<(), DynError> {
    let mut models = runtime.state().hub.subscribe();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let found = models.borrow().spaces.iter().any(|space| {
                !space.stale
                    && space.stars.iter().any(|star| {
                        star.number == 70 && star.title == "M2 controlled process evidence"
                    })
            });
            if found {
                return Ok::<(), DynError>(());
            }
            models.changed().await?;
        }
    })
    .await?
}

async fn get_model_over_http(cockpit_url: &str) -> Result<String, DynError> {
    let url = url::Url::parse(cockpit_url)?;
    let host = url.host_str().ok_or("cockpit URL omitted its host")?;
    let port = url
        .port_or_known_default()
        .ok_or("cockpit URL omitted its port")?;
    let query = url
        .query()
        .map(|query| format!("?{query}"))
        .unwrap_or_default();
    let mut stream = tokio::net::TcpStream::connect((host, port)).await?;
    let request = format!(
        "GET /api/model{query} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).await?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await?;
    let response = String::from_utf8(response)?;
    if !response.starts_with("HTTP/1.1 200 OK") {
        return Err("protected model request was not accepted".into());
    }
    Ok(response)
}

pub async fn run(github_base: String, profile: PathBuf) -> Result<(), DynError> {
    std::fs::create_dir_all(&profile)?;
    let base = url::Url::parse(&github_base)?;
    let controller = DeviceFlowController::new(DeviceFlowClient::new(
        base.clone(),
        "Ov23liWXBEZ0ysYu2MxE",
        "repo",
    )?);
    let credential = wait_for_authorization(&controller).await?;
    println!("APPLICATION_PROCESS_DEVICE_FLOW_PASSED=true");

    let provider = Arc::new(GithubProvider::with_base_uri(
        credential.expose().to_owned(),
        base.as_str(),
    )?);
    let provider_slot = ProviderSlot::new(Arc::new(SignedOut));
    let polling = PollingControl::focus_aware(FOCUSED_POLL_INTERVAL, BACKGROUND_POLL_INTERVAL);
    let entry = SpaceEntry::new(
        RepoRef {
            owner: "teloverge".into(),
            name: "stellr".into(),
        },
        None,
    );
    let runtime = start_runtime_with_entry(
        DesktopRuntimeOptions {
            current_dir: profile.clone(),
            spaces_file: profile.join("spaces.toml"),
            cache_root: profile.join("cache"),
        },
        entry,
        Arc::new(provider_slot.clone()),
        Some(polling.clone()),
    )
    .await?;
    let credential_path = profile.join("github-test-credential");
    let store = Arc::new(TestCredentialStore {
        path: credential_path.clone(),
    });
    let warning = activate_provider_and_store(
        &provider_slot,
        provider,
        runtime.state().refresh.clone(),
        store.clone(),
        credential,
    )
    .await;
    if warning.is_some() || store.load()?.as_deref() != Some("controlled-native-token") {
        return Err("test credential store did not persist the authorized token".into());
    }
    println!("APPLICATION_PROCESS_CREDENTIAL_STORE_PASSED=true");

    wait_for_issue(&runtime).await?;
    println!("APPLICATION_PROCESS_SYNC_PASSED=true");
    let response = get_model_over_http(runtime.cockpit_url()).await?;
    if !response.contains("M2 controlled process evidence") {
        return Err("protected HTTP model omitted the controlled issue".into());
    }
    println!("APPLICATION_PROCESS_HTTP_CONTRACT_PASSED=true");

    let intervals = polling.subscribe();
    let focused = intervals.borrow().as_secs();
    polling.set_focused(false);
    let background = intervals.borrow().as_secs();
    polling.set_focused(true);
    let refocused = intervals.borrow().as_secs();
    if (focused, background, refocused) != (30, 300, 30) {
        return Err("focus-aware polling did not expose the production cadence".into());
    }
    println!("APPLICATION_PROCESS_FOCUS_CADENCE={focused},{background},{refocused}");

    runtime.shutdown_handle().shutdown();
    runtime.wait().await?;
    Ok(())
}
