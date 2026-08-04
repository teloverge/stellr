use std::{path::Path, process::Command, sync::Arc};

use stellr_app::desktop::{DesktopRuntimeOptions, start_runtime};
use stellr_core::{Provider, ProviderError, RawIssue, RepoRef};

struct EmptyProvider;

#[async_trait::async_trait]
impl Provider for EmptyProvider {
    async fn fetch(&self, _repo: &RepoRef) -> Result<Vec<RawIssue>, ProviderError> {
        Ok(vec![])
    }
}

fn git(repo: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(arguments)
        .status()
        .unwrap();
    assert!(status.success(), "git command failed: {arguments:?}");
}

#[tokio::test]
async fn desktop_runtime_opens_and_persists_the_callers_github_repository() {
    let profile = tempfile::tempdir().unwrap();
    let repo = profile.path().join("current-repo");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "--quiet"]);
    git(
        &repo,
        &[
            "remote",
            "add",
            "origin",
            "git@github.com:teloverge/stellr.git",
        ],
    );

    let runtime = start_runtime(
        DesktopRuntimeOptions {
            current_dir: repo.clone(),
            spaces_file: profile.path().join("spaces.toml"),
            cache_root: profile.path().join("cache"),
        },
        Arc::new(EmptyProvider),
    )
    .await
    .unwrap();

    let state = runtime.state();
    let spaces = state.spaces.lock().await;
    assert_eq!(spaces.entries().len(), 1);
    assert_eq!(spaces.entries()[0].repo.slug(), "teloverge/stellr");
    assert_eq!(spaces.entries()[0].path.as_deref(), Some(repo.as_path()));
    drop(spaces);

    runtime.shutdown_handle().shutdown();
    runtime.wait().await.unwrap();
}
