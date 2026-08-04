//! Normalization for every desktop navigation target.

use std::{num::NonZeroU64, path::PathBuf};

use serde::Serialize;
use stellr_core::RepoRef;
use stellr_server::spaces::{SpaceEntry, detect_repo};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RouteTarget {
    pub space_id: String,
    pub repo: String,
    pub path: Option<PathBuf>,
    pub issue: Option<u64>,
}

impl RouteTarget {
    pub fn entry(&self) -> SpaceEntry {
        let (owner, name) = self
            .repo
            .split_once('/')
            .expect("normalized repository should contain one slash");
        SpaceEntry::new(
            RepoRef {
                owner: owner.to_owned(),
                name: name.to_owned(),
            },
            self.path.clone(),
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TargetError {
    #[error("target is empty")]
    Empty,
    #[error("unsupported target `{0}`")]
    Unsupported(String),
    #[error("invalid repository path `{path}`: {message}")]
    Path { path: String, message: String },
    #[error("invalid issue number `{0}`")]
    Issue(String),
}

pub struct TargetResolver {
    cwd: PathBuf,
}

impl TargetResolver {
    pub fn new(cwd: PathBuf) -> Self {
        Self { cwd }
    }

    pub fn resolve(&self, raw: &str) -> Result<RouteTarget, TargetError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(TargetError::Empty);
        }

        let candidate = PathBuf::from(raw);
        let candidate = if candidate.is_absolute() {
            candidate
        } else {
            self.cwd.join(candidate)
        };
        if candidate.exists() {
            return self.resolve_path(candidate, None);
        }

        if raw.starts_with("stellr:") {
            return self.resolve_stellr_link(raw);
        }
        if raw.starts_with("http:") || raw.starts_with("https:") {
            return self.resolve_github_url(raw);
        }
        if let Some(repo) = parse_slug(raw) {
            return Ok(route(repo, None, None));
        }

        if PathBuf::from(raw).is_absolute() || raw.starts_with('.') || raw.contains('\\') {
            return Err(TargetError::Path {
                path: candidate.display().to_string(),
                message: "path does not exist".to_owned(),
            });
        }
        Err(TargetError::Unsupported(raw.to_owned()))
    }

    fn resolve_path(&self, path: PathBuf, issue: Option<u64>) -> Result<RouteTarget, TargetError> {
        let repo = detect_repo(&path).map_err(|message| TargetError::Path {
            path: path.display().to_string(),
            message,
        })?;
        Ok(route(repo, Some(path), issue))
    }

    fn resolve_github_url(&self, raw: &str) -> Result<RouteTarget, TargetError> {
        let url = Url::parse(raw).map_err(|_| TargetError::Unsupported(raw.to_owned()))?;
        if url.scheme() != "https" || url.host_str() != Some("github.com") {
            return Err(TargetError::Unsupported(raw.to_owned()));
        }
        let segments = url
            .path_segments()
            .map(|segments| {
                segments
                    .filter(|segment| !segment.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let (owner, name, issue) = match segments.as_slice() {
            [owner, name] => (*owner, name.trim_end_matches(".git"), None),
            [owner, name, "issues", issue] => (*owner, *name, Some(parse_issue(issue)?)),
            _ => return Err(TargetError::Unsupported(raw.to_owned())),
        };
        let slug = format!("{owner}/{name}");
        let repo = parse_slug(&slug).ok_or_else(|| TargetError::Unsupported(raw.to_owned()))?;
        Ok(route(repo, None, issue))
    }

    fn resolve_stellr_link(&self, raw: &str) -> Result<RouteTarget, TargetError> {
        let url = Url::parse(raw).map_err(|_| TargetError::Unsupported(raw.to_owned()))?;
        if url.scheme() != "stellr" || url.host_str() != Some("space") {
            return Err(TargetError::Unsupported(raw.to_owned()));
        }
        let mut repo = None;
        let mut path = None;
        let mut issue = None;
        for (name, value) in url.query_pairs() {
            match name.as_ref() {
                "repo" if repo.is_none() => repo = Some(value.into_owned()),
                "path" if path.is_none() => path = Some(value.into_owned()),
                "issue" if issue.is_none() => issue = Some(parse_issue(&value)?),
                _ => return Err(TargetError::Unsupported(raw.to_owned())),
            }
        }
        match (repo, path) {
            (Some(repo), None) => parse_slug(&repo)
                .map(|repo| route(repo, None, issue))
                .ok_or_else(|| TargetError::Unsupported(raw.to_owned())),
            (None, Some(path)) => {
                let path = PathBuf::from(path);
                let path = if path.is_absolute() {
                    path
                } else {
                    self.cwd.join(path)
                };
                self.resolve_path(path, issue)
            }
            _ => Err(TargetError::Unsupported(raw.to_owned())),
        }
    }
}

fn route(repo: RepoRef, path: Option<PathBuf>, issue: Option<u64>) -> RouteTarget {
    RouteTarget {
        space_id: format!("{}-{}", repo.owner, repo.name),
        repo: repo.slug(),
        path,
        issue,
    }
}

fn parse_slug(raw: &str) -> Option<RepoRef> {
    let mut segments = raw.split('/');
    let owner = segments.next()?;
    let name = segments.next()?.trim_end_matches(".git");
    if segments.next().is_some() || !valid_slug_part(owner) || !valid_slug_part(name) {
        return None;
    }
    Some(RepoRef {
        owner: owner.to_owned(),
        name: name.to_owned(),
    })
}

fn valid_slug_part(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn parse_issue(raw: &str) -> Result<u64, TargetError> {
    raw.parse::<NonZeroU64>()
        .map(NonZeroU64::get)
        .map_err(|_| TargetError::Issue(raw.to_owned()))
}
