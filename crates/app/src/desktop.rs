use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use serde::Serialize;
use stellr_core::{Provider, ProviderError, RawIssue, RepoRef};
use stellr_github::{
    auth::resolve_token,
    cache::Cache,
    credentials::{CredentialStore, OsCredentialStore},
    device_flow::{DeviceFlowClient, DeviceFlowController, DeviceFlowStatus},
    sync::GithubProvider,
};
use stellr_server::poll::PollingControl;
use stellr_server::spaces::{SpaceEntry, SpaceStore, detect_repo};
use tauri::{Manager, Runtime, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use thiserror::Error;
use tokio::sync::{Mutex, Notify};

use crate::{
    auth_activation::activate_provider_and_store,
    runtime::{
        ApplicationRuntime, ProviderSlot, RuntimeError, RuntimeOptions, SessionAuth, start,
        start_with_polling,
    },
    target::{RouteTarget, TargetResolver},
};

const GITHUB_DEVICE_FLOW_BASE: &str = "https://github.com";
const GITHUB_DEVICE_CLIENT_ID: &str = "Ov23liWXBEZ0ysYu2MxE";
const GITHUB_DEVICE_SCOPE: &str = "repo";
const FOCUSED_POLL_INTERVAL: Duration = Duration::from_secs(30);
const BACKGROUND_POLL_INTERVAL: Duration = Duration::from_secs(5 * 60);

pub struct DesktopLaunch {
    pub cwd: PathBuf,
    pub target: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum NativeRouteEvent {
    Target { target: RouteTarget },
    Error { message: String },
}

#[derive(Clone, Default)]
struct RouteInbox {
    pending: Arc<StdMutex<VecDeque<NativeRouteEvent>>>,
}

impl RouteInbox {
    fn push(&self, event: NativeRouteEvent) {
        self.pending
            .lock()
            .expect("route inbox should not be poisoned")
            .push_back(event);
    }

    fn take(&self) -> Option<NativeRouteEvent> {
        self.pending
            .lock()
            .expect("route inbox should not be poisoned")
            .pop_front()
    }
}

pub struct DesktopRuntimeOptions {
    pub current_dir: PathBuf,
    pub spaces_file: PathBuf,
    pub cache_root: PathBuf,
}

#[derive(Debug, Error)]
pub enum DesktopRuntimeError {
    #[error("could not open the current repository: {0}")]
    CurrentRepository(String),
    #[error("could not save the current repository: {0}")]
    SaveSpace(#[source] std::io::Error),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
}

#[derive(Debug, Error)]
pub enum DesktopHostError {
    #[error("native desktop shell failed: {0}")]
    Tauri(#[from] tauri::Error),
}

struct DesktopState {
    _runtime: ApplicationRuntime,
}

struct DesktopAuthState {
    controller: DeviceFlowController,
    credential_present: AtomicBool,
    provider_slot: ProviderSlot,
    refresh: Arc<Notify>,
    credential_store: Arc<dyn CredentialStore>,
    completion: Mutex<()>,
    storage_warning: Mutex<Option<String>>,
}

impl DesktopAuthState {
    fn new(
        controller: DeviceFlowController,
        credential_present: bool,
        provider_slot: ProviderSlot,
        refresh: Arc<Notify>,
        credential_store: Arc<dyn CredentialStore>,
    ) -> Self {
        Self {
            controller,
            credential_present: AtomicBool::new(credential_present),
            provider_slot,
            refresh,
            credential_store,
            completion: Mutex::new(()),
            storage_warning: Mutex::new(None),
        }
    }

    async fn public_status(&self) -> Result<DeviceFlowStatus, ProviderError> {
        let _completion = self.completion.lock().await;
        let status = self.controller.status().await;
        if matches!(status, DeviceFlowStatus::Authorized { .. })
            && !self.credential_present.load(Ordering::Acquire)
            && let Some(token) = self.controller.take_token().await
        {
            let provider = Arc::new(GithubProvider::new(token.expose().to_owned())?);
            let warning = activate_provider_and_store(
                &self.provider_slot,
                provider,
                self.refresh.clone(),
                self.credential_store.clone(),
                token,
            )
            .await;
            *self.storage_warning.lock().await = warning;
            self.credential_present.store(true, Ordering::Release);
        }

        if self.credential_present.load(Ordering::Acquire)
            && matches!(
                status,
                DeviceFlowStatus::Idle | DeviceFlowStatus::Authorized { .. }
            )
        {
            Ok(DeviceFlowStatus::Authorized {
                storage_warning: self.storage_warning.lock().await.clone(),
            })
        } else {
            Ok(status)
        }
    }
}

struct SignedOutProvider;

#[async_trait::async_trait]
impl Provider for SignedOutProvider {
    async fn fetch(&self, _repo: &RepoRef) -> Result<Vec<RawIssue>, ProviderError> {
        Err(ProviderError::Auth("GitHub sign-in required".to_owned()))
    }
}

fn provider_from_environment() -> Result<(Arc<dyn Provider + Send + Sync>, bool), ProviderError> {
    match resolve_token() {
        Ok(token) => Ok((Arc::new(GithubProvider::new(token)?), true)),
        Err(_) => Ok((Arc::new(SignedOutProvider), false)),
    }
}

fn device_flow_controller() -> Result<DeviceFlowController, Box<dyn std::error::Error + Send + Sync>>
{
    let client = DeviceFlowClient::new(
        GITHUB_DEVICE_FLOW_BASE.parse()?,
        GITHUB_DEVICE_CLIENT_ID,
        GITHUB_DEVICE_SCOPE,
    )?;
    Ok(DeviceFlowController::new(client))
}

fn forwarded_route_event(args: &[String], cwd: &str) -> NativeRouteEvent {
    let forwarded = args.get(1..).unwrap_or_default();
    let raw = match forwarded {
        [] => cwd.to_owned(),
        [command, target] if command == "open" => target.clone(),
        [target] if target.starts_with("stellr:") => target.clone(),
        _ => {
            return NativeRouteEvent::Error {
                message: "Use `stellr` or `stellr open <path|url>` to route the running app."
                    .to_owned(),
            };
        }
    };
    match TargetResolver::new(PathBuf::from(cwd)).resolve(&raw) {
        Ok(target) => NativeRouteEvent::Target { target },
        Err(error) => NativeRouteEvent::Error {
            message: error.to_string(),
        },
    }
}

#[tauri::command]
fn take_route_event(state: State<'_, RouteInbox>) -> Option<NativeRouteEvent> {
    state.take()
}

#[tauri::command]
async fn begin_device_authorization(
    state: State<'_, DesktopAuthState>,
) -> Result<DeviceFlowStatus, String> {
    state
        .controller
        .begin()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn device_authorization_status(
    state: State<'_, DesktopAuthState>,
) -> Result<DeviceFlowStatus, String> {
    state
        .public_status()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn cancel_device_authorization(state: State<'_, DesktopAuthState>) -> Result<(), String> {
    state.controller.cancel().await;
    Ok(())
}

pub async fn start_runtime(
    options: DesktopRuntimeOptions,
    provider: Arc<dyn Provider + Send + Sync>,
) -> Result<ApplicationRuntime, DesktopRuntimeError> {
    let repo = detect_repo(&options.current_dir).map_err(DesktopRuntimeError::CurrentRepository)?;
    let entry = SpaceEntry::new(repo, Some(options.current_dir.clone()));
    start_runtime_with_entry(options, entry, provider, None).await
}

async fn start_runtime_with_entry(
    options: DesktopRuntimeOptions,
    entry: SpaceEntry,
    provider: Arc<dyn Provider + Send + Sync>,
    polling: Option<PollingControl>,
) -> Result<ApplicationRuntime, DesktopRuntimeError> {
    let mut spaces = SpaceStore::load(options.spaces_file.clone());
    if !spaces
        .entries()
        .iter()
        .any(|existing| existing.id == entry.id)
    {
        spaces
            .add(entry)
            .map_err(DesktopRuntimeError::CurrentRepository)?;
        spaces.save().map_err(DesktopRuntimeError::SaveSpace)?;
    }

    let runtime_options = RuntimeOptions {
        address: "127.0.0.1:0".into(),
        session_auth: SessionAuth::Required,
        issue: None,
        spaces_file: options.spaces_file,
        cache_root: options.cache_root,
        poll_interval: FOCUSED_POLL_INTERVAL,
    };
    match polling {
        Some(polling) => start_with_polling(runtime_options, provider, polling).await,
        None => start(runtime_options, provider).await,
    }
    .map_err(DesktopRuntimeError::Runtime)
}

pub fn default_options(current_dir: PathBuf) -> DesktopRuntimeOptions {
    DesktopRuntimeOptions {
        current_dir,
        spaces_file: SpaceStore::default_file(),
        cache_root: Cache::default_root(),
    }
}

pub fn create_main_window<R: Runtime, M: Manager<R>>(
    manager: &M,
    url: tauri::Url,
) -> tauri::Result<WebviewWindow<R>> {
    WebviewWindowBuilder::new(manager, "main", WebviewUrl::External(url))
        .title("Stellr")
        .decorations(true)
        .inner_size(1180.0, 760.0)
        .build()
}

fn route_url(mut url: tauri::Url, target: &RouteTarget) -> tauri::Url {
    let mut fragment = url::form_urlencoded::Serializer::new(String::new());
    fragment.append_pair("s", &target.space_id);
    if let Some(issue) = target.issue {
        fragment.append_pair("i", &issue.to_string());
    }
    url.set_fragment(Some(&fragment.finish()));
    url
}

pub fn run(launch: DesktopLaunch) -> Result<(), DesktopHostError> {
    let route_inbox = RouteInbox::default();
    tauri::Builder::default()
        .manage(route_inbox.clone())
        .plugin(tauri_plugin_single_instance::init(move |app, args, cwd| {
            route_inbox.push(forwarded_route_event(&args, &cwd));
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let startup = tauri::async_runtime::block_on(async {
                let target = TargetResolver::new(launch.cwd.clone()).resolve(&launch.target)?;
                let (provider, credential_present) = provider_from_environment()?;
                let provider_slot = ProviderSlot::new(provider);
                let polling =
                    PollingControl::focus_aware(FOCUSED_POLL_INTERVAL, BACKGROUND_POLL_INTERVAL);
                let runtime = start_runtime_with_entry(
                    default_options(launch.cwd.clone()),
                    target.entry(),
                    Arc::new(provider_slot.clone()),
                    Some(polling),
                )
                .await?;
                let auth = DesktopAuthState::new(
                    device_flow_controller()?,
                    credential_present,
                    provider_slot,
                    runtime.state().refresh.clone(),
                    Arc::new(OsCredentialStore::default()),
                );
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>((runtime, auth, target))
            });

            let (runtime, auth, target) = match startup {
                Ok(startup) => startup,
                Err(error) => {
                    app.dialog()
                        .message(error.to_string())
                        .kind(MessageDialogKind::Error)
                        .title("Stellr could not start")
                        .blocking_show();
                    return Err(error);
                }
            };

            let url = route_url(runtime.cockpit_url().parse()?, &target);
            let window = match create_main_window(app, url) {
                Ok(window) => window,
                Err(error) => {
                    app.dialog()
                        .message(error.to_string())
                        .kind(MessageDialogKind::Error)
                        .title("Stellr could not open its window")
                        .blocking_show();
                    return Err(Box::new(error));
                }
            };
            let polling = runtime.polling_control();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::Focused(focused) = event {
                    polling.set_focused(*focused);
                }
            });
            app.manage(DesktopState { _runtime: runtime });
            app.manage(auth);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            begin_device_authorization,
            device_authorization_status,
            cancel_device_authorization,
            take_route_event
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwarded_open_and_protocol_arguments_share_the_target_resolver() {
        let cwd = std::env::current_dir().unwrap();
        let cwd = cwd.to_string_lossy();
        let open = forwarded_route_event(
            &[
                "stellr.exe".into(),
                "open".into(),
                "https://github.com/teloverge/stellr/issues/62".into(),
            ],
            &cwd,
        );
        let protocol = forwarded_route_event(
            &[
                "stellr.exe".into(),
                "stellr://space?repo=teloverge%2Fstellr&issue=62".into(),
            ],
            &cwd,
        );

        assert!(matches!(
            open,
            NativeRouteEvent::Target {
                target: RouteTarget {
                    issue: Some(62),
                    ..
                }
            }
        ));
        assert!(matches!(
            protocol,
            NativeRouteEvent::Target {
                target: RouteTarget {
                    issue: Some(62),
                    ..
                }
            }
        ));
    }

    #[test]
    fn invalid_forwarded_arguments_queue_an_error_instead_of_a_target() {
        let event = forwarded_route_event(
            &[
                "stellr.exe".into(),
                "open".into(),
                "one".into(),
                "two".into(),
            ],
            ".",
        );
        assert!(matches!(event, NativeRouteEvent::Error { .. }));
    }

    #[test]
    fn initial_route_fragment_preserves_the_authenticated_loopback_query() {
        let url = route_url(
            "http://127.0.0.1:49152/?token=session-token"
                .parse()
                .unwrap(),
            &RouteTarget {
                space_id: "teloverge-stellr".into(),
                repo: "teloverge/stellr".into(),
                path: None,
                issue: Some(62),
            },
        );

        assert_eq!(
            url.as_str(),
            "http://127.0.0.1:49152/?token=session-token#s=teloverge-stellr&i=62"
        );
    }
}
