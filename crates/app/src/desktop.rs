use std::{path::PathBuf, sync::Arc, time::Duration};

use stellr_core::Provider;
use stellr_github::{auth::resolve_token, cache::Cache, sync::GithubProvider};
use stellr_server::spaces::{SpaceEntry, SpaceStore, detect_repo};
use tauri::{Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use thiserror::Error;

use crate::runtime::{ApplicationRuntime, RuntimeError, RuntimeOptions, SessionAuth, start};

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

pub async fn start_runtime(
    options: DesktopRuntimeOptions,
    provider: Arc<dyn Provider + Send + Sync>,
) -> Result<ApplicationRuntime, DesktopRuntimeError> {
    let repo = detect_repo(&options.current_dir).map_err(DesktopRuntimeError::CurrentRepository)?;
    let entry = SpaceEntry::new(repo, Some(options.current_dir));
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

    start(
        RuntimeOptions {
            address: "127.0.0.1:0".into(),
            session_auth: SessionAuth::Required,
            issue: None,
            spaces_file: options.spaces_file,
            cache_root: options.cache_root,
            poll_interval: Duration::from_secs(30),
        },
        provider,
    )
    .await
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

pub fn run(current_dir: PathBuf) -> Result<(), DesktopHostError> {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let startup = tauri::async_runtime::block_on(async {
                let provider_token = resolve_token()?;
                let provider = Arc::new(GithubProvider::new(provider_token)?);
                let runtime = start_runtime(default_options(current_dir.clone()), provider).await?;
                Ok::<ApplicationRuntime, Box<dyn std::error::Error + Send + Sync>>(runtime)
            });

            let runtime = match startup {
                Ok(runtime) => runtime,
                Err(error) => {
                    app.dialog()
                        .message(error.to_string())
                        .kind(MessageDialogKind::Error)
                        .title("Stellr could not start")
                        .blocking_show();
                    return Err(error);
                }
            };

            let url = runtime.cockpit_url().parse()?;
            if let Err(error) = create_main_window(app, url) {
                app.dialog()
                    .message(error.to_string())
                    .kind(MessageDialogKind::Error)
                    .title("Stellr could not open its window")
                    .blocking_show();
                return Err(Box::new(error));
            }
            app.manage(DesktopState { _runtime: runtime });
            Ok(())
        })
        .run(tauri::generate_context!())?;
    Ok(())
}
