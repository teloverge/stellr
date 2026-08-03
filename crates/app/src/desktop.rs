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
use tauri::{
    AppHandle, Manager, Runtime, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    menu::{Menu, MenuItem},
    tray::{TrayIcon, TrayIconBuilder},
};
#[cfg(target_os = "macos")]
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;
use thiserror::Error;
use tokio::sync::{Mutex, Notify};

use crate::{
    auth_activation::activate_provider_and_store,
    route_state::{PersistedRoute, RouteStateStore},
    runtime::{
        ApplicationRuntime, ProviderSlot, RuntimeError, RuntimeOptions, SessionAuth, start,
        start_with_polling,
    },
    target::{RouteTarget, TargetResolver},
    theme::{ThemePreference, ThemeStore},
};

const GITHUB_DEVICE_FLOW_BASE: &str = "https://github.com";
const GITHUB_DEVICE_CLIENT_ID: &str = "Ov23liWXBEZ0ysYu2MxE";
const GITHUB_DEVICE_SCOPE: &str = "repo";
pub(crate) const FOCUSED_POLL_INTERVAL: Duration = Duration::from_secs(30);
pub(crate) const BACKGROUND_POLL_INTERVAL: Duration = Duration::from_secs(5 * 60);

pub struct DesktopLaunch {
    pub cwd: PathBuf,
    pub target: String,
    pub restore_route: bool,
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
    _tray: TrayIcon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayAction {
    Open,
    Quit,
}

fn tray_action(id: &str) -> Option<TrayAction> {
    match id {
        "open" => Some(TrayAction::Open),
        "quit" => Some(TrayAction::Quit),
        _ => None,
    }
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn create_tray(app: &tauri::App) -> tauri::Result<TrayIcon> {
    let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut builder = TrayIconBuilder::new()
        .tooltip("Stellr")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match tray_action(event.id().as_ref()) {
            Some(TrayAction::Open) => show_main_window(app),
            Some(TrayAction::Quit) => app.exit(0),
            None => {}
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)
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

#[cfg(any(test, target_os = "macos"))]
trait DeepLinkSource {
    type Error;

    fn current_urls(&self) -> Result<Option<Vec<url::Url>>, Self::Error>;
    fn on_open_url(&self, handler: Box<dyn Fn(Vec<url::Url>) + Send + Sync>);
}

#[cfg(target_os = "macos")]
struct TauriDeepLinks<R: Runtime>(AppHandle<R>);

#[cfg(target_os = "macos")]
impl<R: Runtime> DeepLinkSource for TauriDeepLinks<R> {
    type Error = tauri_plugin_deep_link::Error;

    fn current_urls(&self) -> Result<Option<Vec<url::Url>>, Self::Error> {
        self.0.deep_link().get_current()
    }

    fn on_open_url(&self, handler: Box<dyn Fn(Vec<url::Url>) + Send + Sync>) {
        self.0
            .deep_link()
            .on_open_url(move |event| handler(event.urls()));
    }
}

#[cfg(any(test, target_os = "macos"))]
fn queue_deep_link_urls(
    inbox: &RouteInbox,
    cwd: &std::path::Path,
    urls: Vec<url::Url>,
    reveal: &Arc<dyn Fn() + Send + Sync>,
) {
    if urls.is_empty() {
        return;
    }
    for url in urls {
        let event = match TargetResolver::new(cwd.to_path_buf()).resolve(url.as_str()) {
            Ok(target) => NativeRouteEvent::Target { target },
            Err(error) => NativeRouteEvent::Error {
                message: error.to_string(),
            },
        };
        inbox.push(event);
    }
    reveal();
}

#[cfg(any(test, target_os = "macos"))]
fn install_deep_link_routing<S: DeepLinkSource>(
    source: &S,
    inbox: RouteInbox,
    cwd: PathBuf,
    reveal: Arc<dyn Fn() + Send + Sync>,
) -> Result<(), S::Error> {
    if let Some(urls) = source.current_urls()? {
        queue_deep_link_urls(&inbox, &cwd, urls, &reveal);
    }
    source.on_open_url(Box::new(move |urls| {
        queue_deep_link_urls(&inbox, &cwd, urls, &reveal);
    }));
    Ok(())
}

#[tauri::command]
fn take_route_event(state: State<'_, RouteInbox>) -> Option<NativeRouteEvent> {
    state.take()
}

#[tauri::command]
fn persist_route_state(
    state: State<'_, RouteStateStore>,
    space: Option<String>,
    issue: Option<u64>,
) -> Result<(), String> {
    let route = match space {
        Some(space) => PersistedRoute::new(space, issue)
            .ok_or_else(|| "route state must contain a space and a positive issue".to_owned())?
            .into(),
        None if issue.is_none() => None,
        None => return Err("an issue route requires a space".to_owned()),
    };
    state.save(route).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_theme_preference(state: State<'_, ThemeStore>) -> ThemePreference {
    state.load()
}

#[tauri::command]
fn set_theme_preference(
    state: State<'_, ThemeStore>,
    preference: ThemePreference,
) -> Result<(), String> {
    state.save(preference).map_err(|error| error.to_string())
}

#[tauri::command]
async fn choose_repository_directory(app: AppHandle) -> Result<Option<String>, String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Choose a Git repository")
        .pick_folder(move |folder| {
            let _ = sender.send(folder);
        });
    let folder = receiver
        .await
        .map_err(|_| "repository chooser closed unexpectedly".to_owned())?;
    folder
        .map(|folder| {
            folder
                .into_path()
                .map(|path| path.to_string_lossy().into_owned())
                .map_err(|error| error.to_string())
        })
        .transpose()
}

fn validated_external_url(raw: &str) -> Result<url::Url, String> {
    let url = url::Url::parse(raw).map_err(|_| "external URL is invalid".to_owned())?;
    if url.scheme() != "https" {
        return Err("only https external URLs are allowed".to_owned());
    }
    Ok(url)
}

#[tauri::command]
fn open_external_url(app: AppHandle, url: String) -> Result<(), String> {
    let url = validated_external_url(&url)?;
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|error| error.to_string())
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

pub(crate) async fn start_runtime_with_entry(
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
        .visible(false)
        .build()
}

fn route_url(mut url: tauri::Url, route: &PersistedRoute) -> tauri::Url {
    let mut fragment = url::form_urlencoded::Serializer::new(String::new());
    fragment.append_pair("s", &route.space);
    if let Some(issue) = route.issue {
        fragment.append_pair("i", &issue.to_string());
    }
    url.set_fragment(Some(&fragment.finish()));
    url
}

fn initial_route(
    target: &RouteTarget,
    restored: Option<PersistedRoute>,
    restore_route: bool,
) -> PersistedRoute {
    if restore_route && let Some(restored) = restored {
        return restored;
    }
    PersistedRoute::new(target.space_id.clone(), target.issue)
        .expect("a resolved target always has a space")
}

pub fn run(launch: DesktopLaunch) -> Result<(), DesktopHostError> {
    let route_inbox = RouteInbox::default();
    let route_state = RouteStateStore::new(RouteStateStore::default_file());
    let theme_state = ThemeStore::new(ThemeStore::default_file());
    tauri::Builder::default()
        .manage(route_inbox.clone())
        .manage(route_state.clone())
        .manage(theme_state)
        .plugin(tauri_plugin_single_instance::init(move |app, args, cwd| {
            route_inbox.push(forwarded_route_event(&args, &cwd));
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                )
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
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

            let startup_route = initial_route(&target, route_state.load(), launch.restore_route);
            let url = route_url(runtime.cockpit_url().parse()?, &startup_route);
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
            #[cfg(target_os = "macos")]
            {
                let app_handle = app.handle().clone();
                install_deep_link_routing(
                    &TauriDeepLinks(app.handle().clone()),
                    app.state::<RouteInbox>().inner().clone(),
                    launch.cwd.clone(),
                    Arc::new(move || show_main_window(&app_handle)),
                )?;
            }
            let polling = runtime.polling_control();
            let app_handle = window.app_handle().clone();
            window.on_window_event(move |event| match event {
                tauri::WindowEvent::Focused(focused) => polling.set_focused(*focused),
                tauri::WindowEvent::CloseRequested { .. } => app_handle.exit(0),
                _ => {}
            });
            window.show()?;
            let tray = create_tray(app)?;
            app.manage(DesktopState {
                _runtime: runtime,
                _tray: tray,
            });
            app.manage(auth);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            begin_device_authorization,
            device_authorization_status,
            cancel_device_authorization,
            take_route_event,
            persist_route_state,
            get_theme_preference,
            set_theme_preference,
            choose_repository_directory,
            open_external_url
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakeDeepLinks {
        current: Vec<url::Url>,
        opened: Vec<url::Url>,
    }

    impl DeepLinkSource for FakeDeepLinks {
        type Error = std::convert::Infallible;

        fn current_urls(&self) -> Result<Option<Vec<url::Url>>, Self::Error> {
            Ok(Some(self.current.clone()))
        }

        fn on_open_url(&self, handler: Box<dyn Fn(Vec<url::Url>) + Send + Sync>) {
            handler(self.opened.clone());
        }
    }

    #[test]
    fn startup_and_running_deep_links_route_and_reveal_the_app() {
        let source = FakeDeepLinks {
            current: vec![
                "stellr://space?repo=teloverge%2Fstellr&issue=61"
                    .parse()
                    .unwrap(),
            ],
            opened: vec![
                "stellr://space?repo=teloverge%2Fstellr&issue=62"
                    .parse()
                    .unwrap(),
            ],
        };
        let inbox = RouteInbox::default();
        let reveals = Arc::new(AtomicUsize::new(0));
        let reveal_count = reveals.clone();

        install_deep_link_routing(
            &source,
            inbox.clone(),
            PathBuf::from("D:\\dev\\stellr"),
            Arc::new(move || {
                reveal_count.fetch_add(1, Ordering::SeqCst);
            }),
        )
        .unwrap();

        assert!(matches!(
            inbox.take(),
            Some(NativeRouteEvent::Target {
                target: RouteTarget {
                    issue: Some(61),
                    ..
                }
            })
        ));
        assert!(matches!(
            inbox.take(),
            Some(NativeRouteEvent::Target {
                target: RouteTarget {
                    issue: Some(62),
                    ..
                }
            })
        ));
        assert_eq!(reveals.load(Ordering::SeqCst), 2);
    }

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
            &PersistedRoute::new("teloverge-stellr", Some(62)).unwrap(),
        );

        assert_eq!(
            url.as_str(),
            "http://127.0.0.1:49152/?token=session-token#s=teloverge-stellr&i=62"
        );
    }

    #[test]
    fn bare_launch_restores_but_explicit_targets_win() {
        let target = RouteTarget {
            space_id: "explicit-space".into(),
            repo: "teloverge/explicit-space".into(),
            path: None,
            issue: Some(70),
        };
        let restored = PersistedRoute::new("remembered-space", Some(64)).unwrap();

        assert_eq!(
            initial_route(&target, Some(restored.clone()), true),
            restored
        );
        assert_eq!(
            initial_route(&target, Some(restored), false),
            PersistedRoute::new("explicit-space", Some(70)).unwrap()
        );
    }

    #[test]
    fn tray_menu_exposes_only_open_and_quit_lifecycle_actions() {
        assert_eq!(tray_action("open"), Some(TrayAction::Open));
        assert_eq!(tray_action("quit"), Some(TrayAction::Quit));
        assert_eq!(tray_action("hide"), None);
    }

    #[test]
    fn external_opening_accepts_https_and_rejects_other_schemes() {
        assert_eq!(
            validated_external_url("https://github.com/teloverge/stellr")
                .unwrap()
                .scheme(),
            "https"
        );
        assert!(validated_external_url("http://github.com/teloverge/stellr").is_err());
        assert!(validated_external_url("javascript:alert(1)").is_err());
        assert!(validated_external_url("not a url").is_err());
    }
}
