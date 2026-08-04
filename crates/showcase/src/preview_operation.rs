use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use stellr_core::RepoRef;
use thiserror::Error;

use crate::preview::{MAX_MANIFEST_BYTES, MAX_PNG_BYTES};
use crate::{
    LiveReleaseRequest, MAX_SVG_BYTES, PreviewRenderError, ReleaseHistoryError,
    ReleaseHistorySource, ReleaseStory, StaticPreview, render_static_preview, validate_svg_safety,
};

const SVG_NAME: &str = "release.svg";
const PNG_NAME: &str = "release.png";
const MANIFEST_NAME: &str = "story.json";
const REVIEW_NAME: &str = "review.html";
const ARTIFACT_NAMES: [&str; 4] = [SVG_NAME, PNG_NAME, MANIFEST_NAME, REVIEW_NAME];
static NEXT_STAGING_ID: AtomicU64 = AtomicU64::new(0);

/// Pure preview renderer used by the live release operation.
pub trait PreviewRenderer: Send + Sync {
    fn render(&self, story: &ReleaseStory) -> Result<StaticPreview, PreviewRenderError>;
}

/// Production renderer for deterministic SVG, PNG, manifest, and review bytes.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultPreviewRenderer;

impl PreviewRenderer for DefaultPreviewRenderer {
    fn render(&self, story: &ReleaseStory) -> Result<StaticPreview, PreviewRenderError> {
        render_static_preview(story)
    }
}

/// Location of one complete, reviewable live release preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewReceipt {
    pub directory: PathBuf,
    pub digest: String,
}

/// A preview stage failed before a partial preview could be published.
#[derive(Debug, Error)]
pub enum PreviewOperationError {
    #[error("preview release version is not a safe Windows directory name: '{0}'")]
    InvalidReleaseVersion(String),
    #[error("live history stage failed: {0}")]
    History(#[from] ReleaseHistoryError),
    #[error("live story identity stage failed: {0}")]
    Identity(String),
    #[error("render stage failed: {0}")]
    Render(#[from] PreviewRenderError),
    #[error("determinism stage failed: repeated rendering produced different bytes")]
    Nondeterministic,
    #[error("output validation stage '{stage}' failed: {detail}")]
    OutputValidation { stage: &'static str, detail: String },
    #[error("existing preview differs at {path}; previous preview was left untouched")]
    ExistingPreviewDiffers { path: PathBuf },
    #[error("filesystem stage '{stage}' failed at {path}: {source}")]
    Filesystem {
        stage: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Reads one live release story, proves deterministic rendering, and publishes
/// the four review artifacts together under `target/readme-showcase/<version>`.
pub async fn generate_release_preview<S, R>(
    source: &S,
    renderer: &R,
    repository: &RepoRef,
    request: LiveReleaseRequest,
    repository_root: &Path,
) -> Result<PreviewReceipt, PreviewOperationError>
where
    S: ReleaseHistorySource + ?Sized,
    R: PreviewRenderer + ?Sized,
{
    validate_release_component(&request.release_version)?;
    let release_version = request.release_version.clone();
    let milestone_title = request.milestone_title.clone();
    let story = source.build_story(repository, request).await?;
    validate_story_identity(&story, repository, &release_version, &milestone_title)?;

    let first = renderer.render(&story)?;
    let second = renderer.render(&story)?;
    if first != second {
        return Err(PreviewOperationError::Nondeterministic);
    }
    validate_outputs(&story, &first)?;
    let digest = crate::preview_digest(&first);

    let target = repository_root.join("target");
    let preview_root = target.join("readme-showcase");
    reject_reparse_point(repository_root, "repository root")?;
    reject_reparse_point(&target, "target directory")?;
    reject_reparse_point(&preview_root, "preview root")?;
    let destination = preview_root.join(&release_version);
    reject_reparse_point(&destination, "preview destination")?;
    if destination.exists() {
        if existing_preview_matches(&destination, &first)? {
            return Ok(PreviewReceipt {
                directory: destination,
                digest,
            });
        }
        return Err(PreviewOperationError::ExistingPreviewDiffers { path: destination });
    }

    fs::create_dir_all(&preview_root).map_err(|source| PreviewOperationError::Filesystem {
        stage: "create preview root",
        path: preview_root.clone(),
        source,
    })?;
    reject_reparse_point(&target, "target directory")?;
    reject_reparse_point(&preview_root, "preview root")?;
    let staging = create_staging_directory(&preview_root, &release_version)?;
    reject_reparse_point(&staging, "staging directory")?;
    let guard = StagingGuard::new(staging);
    write_and_verify(guard.path(), &first)?;
    fs::rename(guard.path(), &destination).map_err(|source| PreviewOperationError::Filesystem {
        stage: "publish preview directory",
        path: destination.clone(),
        source,
    })?;
    guard.disarm();

    Ok(PreviewReceipt {
        directory: destination,
        digest,
    })
}

fn validate_story_identity(
    story: &ReleaseStory,
    repository: &RepoRef,
    release_version: &str,
    milestone_title: &str,
) -> Result<(), PreviewOperationError> {
    if story.repository != repository.slug() {
        return Err(PreviewOperationError::Identity(format!(
            "expected repository '{}', got '{}'",
            repository.slug(),
            story.repository
        )));
    }
    if story.release_version != release_version {
        return Err(PreviewOperationError::Identity(format!(
            "expected release '{release_version}', got '{}'",
            story.release_version
        )));
    }
    if story.milestone.title != milestone_title {
        return Err(PreviewOperationError::Identity(format!(
            "expected milestone '{milestone_title}', got '{}'",
            story.milestone.title
        )));
    }
    Ok(())
}

pub(crate) fn validate_outputs(
    story: &ReleaseStory,
    preview: &StaticPreview,
) -> Result<(), PreviewOperationError> {
    validate_budget("SVG budget", preview.svg.len(), MAX_SVG_BYTES)?;
    validate_budget("PNG budget", preview.png.len(), MAX_PNG_BYTES)?;
    validate_budget(
        "manifest budget",
        preview.manifest.len(),
        MAX_MANIFEST_BYTES,
    )?;

    let svg = std::str::from_utf8(&preview.svg).map_err(|error| {
        PreviewOperationError::OutputValidation {
            stage: "SVG encoding",
            detail: error.to_string(),
        }
    })?;
    validate_svg_safety(svg).map_err(|error| PreviewOperationError::OutputValidation {
        stage: "SVG safety",
        detail: error.to_string(),
    })?;
    let manifest: ReleaseStory = serde_json::from_slice(&preview.manifest).map_err(|error| {
        PreviewOperationError::OutputValidation {
            stage: "manifest decoding",
            detail: error.to_string(),
        }
    })?;
    if manifest != *story {
        return Err(PreviewOperationError::OutputValidation {
            stage: "manifest agreement",
            detail: "rendered manifest differs from the acquired release story".to_owned(),
        });
    }
    let png = resvg::tiny_skia::Pixmap::decode_png(&preview.png).map_err(|error| {
        PreviewOperationError::OutputValidation {
            stage: "PNG dimensions",
            detail: format!("PNG could not be fully decoded: {error}"),
        }
    })?;
    if png.width() != 1_600 || png.height() != 900 {
        return Err(PreviewOperationError::OutputValidation {
            stage: "PNG dimensions",
            detail: "expected a 1600 by 900 PNG".to_owned(),
        });
    }

    let canonical =
        render_static_preview(story).map_err(|error| PreviewOperationError::OutputValidation {
            stage: "canonical rerender",
            detail: error.to_string(),
        })?;
    if preview.svg != canonical.svg {
        return Err(PreviewOperationError::OutputValidation {
            stage: "SVG semantic agreement",
            detail: "SVG bytes differ from the trusted release-story rendering".to_owned(),
        });
    }
    if preview.png != canonical.png {
        return Err(PreviewOperationError::OutputValidation {
            stage: "PNG final-scene agreement",
            detail: "PNG bytes differ from the trusted final-scene rasterization".to_owned(),
        });
    }
    if preview.manifest != canonical.manifest {
        return Err(PreviewOperationError::OutputValidation {
            stage: "manifest agreement",
            detail: "manifest bytes differ from the trusted release-story rendering".to_owned(),
        });
    }
    if preview.review_html != canonical.review_html {
        return Err(PreviewOperationError::OutputValidation {
            stage: "review page agreement",
            detail: "review page differs from the trusted self-contained rendering".to_owned(),
        });
    }
    Ok(())
}

fn validate_budget(
    stage: &'static str,
    actual: usize,
    limit: usize,
) -> Result<(), PreviewOperationError> {
    if actual > limit {
        return Err(PreviewOperationError::OutputValidation {
            stage,
            detail: format!("{actual} bytes exceeds the {limit} byte limit"),
        });
    }
    Ok(())
}

pub(crate) fn validate_release_component(value: &str) -> Result<(), PreviewOperationError> {
    let invalid_character = value.chars().any(|character| {
        character.is_control()
            || matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            )
    });
    let stem = value
        .split_once('.')
        .map_or(value, |(stem, _)| stem)
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'));
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.ends_with([' ', '.'])
        || invalid_character
        || reserved
    {
        return Err(PreviewOperationError::InvalidReleaseVersion(
            value.to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn reject_reparse_point(
    path: &Path,
    stage: &'static str,
) -> Result<(), PreviewOperationError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_reparse_point(&metadata) => {
            Err(PreviewOperationError::OutputValidation {
                stage,
                detail: format!("{} is a filesystem reparse point", path.display()),
            })
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(PreviewOperationError::Filesystem {
            stage: "inspect output path",
            path: path.to_path_buf(),
            source,
        }),
    }
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn existing_preview_matches(
    destination: &Path,
    preview: &StaticPreview,
) -> Result<bool, PreviewOperationError> {
    reject_reparse_point(destination, "preview destination")?;
    if !destination.is_dir() {
        return Ok(false);
    }
    let expected_names = ARTIFACT_NAMES.into_iter().collect::<BTreeSet<_>>();
    let actual_names = fs::read_dir(destination)
        .map_err(|source| PreviewOperationError::Filesystem {
            stage: "read existing preview",
            path: destination.to_path_buf(),
            source,
        })?
        .map(|entry| {
            entry
                .map_err(|source| PreviewOperationError::Filesystem {
                    stage: "read existing preview entry",
                    path: destination.to_path_buf(),
                    source,
                })
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if actual_names != expected_names.into_iter().map(str::to_owned).collect() {
        return Ok(false);
    }
    for (name, bytes) in artifact_bytes(preview) {
        let path = destination.join(name);
        reject_reparse_point(&path, "existing preview artifact")?;
        match fs::read(&path) {
            Ok(existing) if existing == bytes => {}
            Ok(_) => return Ok(false),
            Err(source) => {
                return Err(PreviewOperationError::Filesystem {
                    stage: "read existing preview artifact",
                    path,
                    source,
                });
            }
        }
    }
    Ok(true)
}

fn create_staging_directory(
    preview_root: &Path,
    release_version: &str,
) -> Result<PathBuf, PreviewOperationError> {
    for _ in 0..16 {
        let id = NEXT_STAGING_ID.fetch_add(1, Ordering::Relaxed);
        let staging = preview_root.join(format!(
            ".{release_version}.preview-{}-{id}",
            std::process::id()
        ));
        match fs::create_dir(&staging) {
            Ok(()) => return Ok(staging),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(PreviewOperationError::Filesystem {
                    stage: "create staging directory",
                    path: staging,
                    source,
                });
            }
        }
    }
    Err(PreviewOperationError::Filesystem {
        stage: "create staging directory",
        path: preview_root.to_path_buf(),
        source: io::Error::new(io::ErrorKind::AlreadyExists, "staging names exhausted"),
    })
}

fn write_and_verify(staging: &Path, preview: &StaticPreview) -> Result<(), PreviewOperationError> {
    for (name, bytes) in artifact_bytes(preview) {
        let path = staging.join(name);
        fs::write(&path, bytes).map_err(|source| PreviewOperationError::Filesystem {
            stage: "write preview artifact",
            path: path.clone(),
            source,
        })?;
        let stored = fs::read(&path).map_err(|source| PreviewOperationError::Filesystem {
            stage: "verify preview artifact",
            path: path.clone(),
            source,
        })?;
        if stored != bytes {
            return Err(PreviewOperationError::OutputValidation {
                stage: "stored artifact verification",
                detail: format!("{} changed while being written", path.display()),
            });
        }
    }
    Ok(())
}

fn artifact_bytes(preview: &StaticPreview) -> [(&'static str, &[u8]); 4] {
    [
        (SVG_NAME, &preview.svg),
        (PNG_NAME, &preview.png),
        (MANIFEST_NAME, &preview.manifest),
        (REVIEW_NAME, &preview.review_html),
    ]
}

struct StagingGuard {
    path: PathBuf,
    armed: std::cell::Cell<bool>,
}

impl StagingGuard {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            armed: std::cell::Cell::new(true),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn disarm(&self) {
        self.armed.set(false);
    }
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        if self.armed.get() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
