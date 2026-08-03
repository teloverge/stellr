use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::{ArgGroup, Args, Parser, Subcommand};
use stellr_core::RepoRef;
use stellr_showcase::{
    DefaultPreviewRenderer, GithubReleaseHistorySource, LiveReleaseRequest, ReleaseWindowStart,
    UtcTimestamp, generate_release_preview,
};
use thiserror::Error;

#[derive(Parser)]
#[command(name = "stellr-showcase")]
#[command(about = "Generate and accept deterministic Stellr README release constellations")]
struct Cli {
    #[command(subcommand)]
    command: ShowcaseCommand,
}

#[derive(Subcommand)]
enum ShowcaseCommand {
    Preview(PreviewArgs),
}

#[derive(Args)]
#[command(group(
    ArgGroup::new("release_start")
        .required(true)
        .args(["from_release", "from_cutoff"])
))]
struct PreviewArgs {
    #[arg(long)]
    milestone: String,
    #[arg(long)]
    version: Option<String>,
    #[arg(long)]
    from_release: Option<String>,
    #[arg(long)]
    from_cutoff: Option<UtcTimestamp>,
    #[arg(long)]
    cutoff: UtcTimestamp,
    #[arg(long)]
    repo: Option<String>,
}

#[derive(Debug, Error)]
enum CliError {
    #[error("current directory could not be read: {0}")]
    CurrentDirectory(#[source] std::io::Error),
    #[error("native git stage '{stage}' failed: {detail}")]
    Git { stage: &'static str, detail: String },
    #[error("repository '{0}' must be owner/name or a supported GitHub remote")]
    Repository(String),
    #[error(transparent)]
    History(#[from] stellr_showcase::ReleaseHistoryError),
    #[error(transparent)]
    Preview(#[from] stellr_showcase::PreviewOperationError),
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        ShowcaseCommand::Preview(args) => preview(args).await,
    }
}

async fn preview(args: PreviewArgs) -> Result<(), CliError> {
    let current = std::env::current_dir().map_err(CliError::CurrentDirectory)?;
    let repository_root = git_repository_root(&current)?;
    let repository = match args.repo {
        Some(value) => parse_repository(&value).ok_or(CliError::Repository(value))?,
        None => github_origin(&repository_root)?,
    };
    let release_version = args.version.unwrap_or_else(|| args.milestone.clone());
    let start = match (args.from_release, args.from_cutoff) {
        (Some(tag), None) => ReleaseWindowStart::PreviousRelease { tag },
        (None, Some(starting_cutoff)) => ReleaseWindowStart::FirstRelease { starting_cutoff },
        _ => unreachable!("clap requires exactly one release boundary"),
    };
    let source = GithubReleaseHistorySource::new()?;
    let receipt = generate_release_preview(
        &source,
        &DefaultPreviewRenderer,
        &repository,
        LiveReleaseRequest {
            release_version,
            milestone_title: args.milestone,
            start,
            ending_cutoff: args.cutoff,
        },
        &repository_root,
    )
    .await?;
    println!("Preview ready: {}", receipt.directory.display());
    Ok(())
}

fn git_repository_root(current: &Path) -> Result<PathBuf, CliError> {
    let output = Command::new("git.exe")
        .arg("-C")
        .arg(current)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| CliError::Git {
            stage: "find repository root",
            detail: error.to_string(),
        })?;
    if !output.status.success() {
        return Err(CliError::Git {
            stage: "find repository root",
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if root.is_empty() {
        return Err(CliError::Git {
            stage: "find repository root",
            detail: "git returned an empty repository root".to_owned(),
        });
    }
    Ok(PathBuf::from(root))
}

fn github_origin(repository_root: &Path) -> Result<RepoRef, CliError> {
    let output = Command::new("git.exe")
        .arg("-C")
        .arg(repository_root)
        .args(["remote", "get-url", "origin"])
        .output()
        .map_err(|error| CliError::Git {
            stage: "read GitHub origin",
            detail: error.to_string(),
        })?;
    if !output.status.success() {
        return Err(CliError::Git {
            stage: "read GitHub origin",
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    parse_repository(&value).ok_or(CliError::Repository(value))
}

fn parse_repository(value: &str) -> Option<RepoRef> {
    let trimmed = value.trim().trim_end_matches('/').trim_end_matches(".git");
    let slug = trimmed
        .strip_prefix("git@github.com:")
        .or_else(|| trimmed.strip_prefix("ssh://git@github.com/"))
        .or_else(|| trimmed.strip_prefix("https://github.com/"))
        .or_else(|| trimmed.strip_prefix("http://github.com/"))
        .unwrap_or(trimmed);
    let mut parts = slug.split('/');
    let owner = parts.next()?.trim();
    let name = parts.next()?.trim();
    if owner.is_empty() || name.is_empty() || parts.next().is_some() {
        return None;
    }
    Some(RepoRef {
        owner: owner.to_owned(),
        name: name.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, ShowcaseCommand, parse_repository};

    #[test]
    fn preview_cli_requires_one_explicit_release_start() {
        let later = Cli::try_parse_from([
            "stellr-showcase",
            "preview",
            "--milestone",
            "v0.2.0",
            "--from-release",
            "v0.1.0",
            "--cutoff",
            "2026-08-02T19:00:00Z",
        ])
        .unwrap();
        let first = Cli::try_parse_from([
            "stellr-showcase",
            "preview",
            "--milestone",
            "M1",
            "--from-cutoff",
            "2026-07-01T00:00:00Z",
            "--cutoff",
            "2026-08-02T19:00:00Z",
        ])
        .unwrap();
        assert!(matches!(later.command, ShowcaseCommand::Preview(_)));
        assert!(matches!(first.command, ShowcaseCommand::Preview(_)));
        assert!(
            Cli::try_parse_from([
                "stellr-showcase",
                "preview",
                "--milestone",
                "M1",
                "--cutoff",
                "2026-08-02T19:00:00Z",
            ])
            .is_err()
        );
    }

    #[test]
    fn repository_parser_accepts_native_git_remote_forms_only() {
        for value in [
            "teloverge/stellr",
            "git@github.com:teloverge/stellr.git",
            "ssh://git@github.com/teloverge/stellr.git",
            "https://github.com/teloverge/stellr.git",
        ] {
            let repo = parse_repository(value).unwrap();
            assert_eq!(repo.slug(), "teloverge/stellr");
        }
        assert!(parse_repository("https://gitlab.com/teloverge/stellr").is_none());
        assert!(parse_repository("too/many/path/parts").is_none());
    }
}
