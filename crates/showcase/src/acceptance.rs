use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use stellr_core::Status;
use thiserror::Error;

use crate::{PreviewOperationError, ReleaseStory, StaticPreview};

const PREVIEW_DIGEST_DOMAIN: &[u8] = b"stellr-release-preview-v1\0";
const README_START: &str = "<!-- stellr-release-constellation:start -->";
const README_END: &str = "<!-- stellr-release-constellation:end -->";
const LEGACY_HEADING: &str = "## Release constellation compatibility probe";
static NEXT_ACCEPTANCE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssetPublication {
    Created,
    Existing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptanceReceipt {
    pub assets: [PathBuf; 3],
    pub readme: PathBuf,
    pub digest: String,
}

#[derive(Debug, Error)]
pub enum AcceptanceError {
    #[error("filesystem stage '{stage}' failed at {path}: {source}")]
    Filesystem {
        stage: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("reviewed preview digest mismatch: expected {expected}, got {actual}")]
    DigestMismatch { expected: String, actual: String },
    #[error("reviewed directory is not the exact four-file preview: {0}")]
    InvalidPreview(String),
    #[error(transparent)]
    Validation(#[from] PreviewOperationError),
    #[error("README showcase contract is invalid: {0}")]
    InvalidReadme(String),
    #[error("versioned showcase asset already exists with different bytes: {0}")]
    AssetConflict(PathBuf),
    #[error("acceptance publication failed: {source}; complete unreferenced assets: {report}")]
    Publication {
        unreferenced_assets: Vec<PathBuf>,
        report: String,
        #[source]
        source: Box<AcceptanceError>,
    },
}

impl AcceptanceError {
    fn publication(unreferenced_assets: Vec<PathBuf>, source: AcceptanceError) -> Self {
        let report = unreferenced_assets
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        Self::Publication {
            unreferenced_assets,
            report,
            source: Box::new(source),
        }
    }

    pub fn unreferenced_assets(&self) -> &[PathBuf] {
        match self {
            Self::Publication {
                unreferenced_assets,
                ..
            } => unreferenced_assets,
            _ => &[],
        }
    }
}

/// Returns the identity a maintainer must explicitly approve before acceptance.
pub fn preview_digest(preview: &StaticPreview) -> String {
    let mut digest = Sha256::new();
    digest.update(PREVIEW_DIGEST_DOMAIN);
    for (name, bytes) in [
        ("release.svg", preview.svg.as_slice()),
        ("release.png", preview.png.as_slice()),
        ("story.json", preview.manifest.as_slice()),
        ("review.html", preview.review_html.as_slice()),
    ] {
        digest.update((name.len() as u64).to_be_bytes());
        digest.update(name.as_bytes());
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(bytes);
    }
    format!("sha256:{:x}", digest.finalize())
}

pub fn accept_release_preview(
    repository_root: &Path,
    preview_directory: &Path,
    expected_digest: &str,
) -> Result<AcceptanceReceipt, AcceptanceError> {
    crate::preview_operation::reject_reparse_point(preview_directory, "preview directory")?;
    let actual_names = fs::read_dir(preview_directory)
        .map_err(|source| AcceptanceError::Filesystem {
            stage: "read reviewed preview directory",
            path: preview_directory.to_path_buf(),
            source,
        })?
        .map(|entry| {
            entry
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .map_err(|source| AcceptanceError::Filesystem {
                    stage: "read reviewed preview entry",
                    path: preview_directory.to_path_buf(),
                    source,
                })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let expected_names = ["release.svg", "release.png", "story.json", "review.html"]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if actual_names != expected_names {
        return Err(AcceptanceError::InvalidPreview(format!(
            "expected {expected_names:?}, got {actual_names:?}"
        )));
    }

    let read = |name: &str| {
        let path = preview_directory.join(name);
        crate::preview_operation::reject_reparse_point(&path, "preview artifact")?;
        fs::read(&path).map_err(|source| AcceptanceError::Filesystem {
            stage: "read reviewed preview",
            path,
            source,
        })
    };
    let preview = StaticPreview {
        svg: read("release.svg")?,
        png: read("release.png")?,
        manifest: read("story.json")?,
        review_html: read("review.html")?,
    };
    let actual = preview_digest(&preview);
    if actual != expected_digest {
        return Err(AcceptanceError::DigestMismatch {
            expected: expected_digest.to_owned(),
            actual,
        });
    }

    let story: ReleaseStory = serde_json::from_slice(&preview.manifest)
        .map_err(|error| AcceptanceError::InvalidPreview(format!("manifest decoding: {error}")))?;
    crate::preview_operation::validate_outputs(&story, &preview)?;
    let directory_version = preview_directory
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| {
            AcceptanceError::InvalidPreview("preview directory has no release name".to_owned())
        })?;
    if directory_version != story.release_version {
        return Err(AcceptanceError::InvalidPreview(format!(
            "reviewed release version '{}' does not match preview directory '{directory_version}'",
            story.release_version
        )));
    }

    crate::preview_operation::validate_release_component(&directory_version)?;

    let readme = repository_root.join("README.md");
    crate::preview_operation::reject_reparse_point(repository_root, "repository root")?;
    crate::preview_operation::reject_reparse_point(&readme, "README")?;
    let original_readme =
        fs::read_to_string(&readme).map_err(|source| AcceptanceError::Filesystem {
            stage: "read current README",
            path: readme.clone(),
            source,
        })?;
    let replacement = replace_showcase_section(&original_readme, &story)?;
    let version_url = encode_url_component(&directory_version);
    let svg_was_referenced =
        original_readme.contains(&format!("docs/assets/readme-showcase/{version_url}.svg"));
    let png_was_referenced =
        original_readme.contains(&format!("docs/assets/readme-showcase/{version_url}.png"));
    let previously_published = [
        svg_was_referenced,
        png_was_referenced,
        svg_was_referenced && png_was_referenced,
    ];

    let docs = repository_root.join("docs");
    let assets = docs.join("assets");
    let asset_root = assets.join("readme-showcase");
    validate_asset_ancestry(repository_root, &docs, &assets, &asset_root)?;
    fs::create_dir_all(&asset_root).map_err(|source| AcceptanceError::Filesystem {
        stage: "create showcase asset directory",
        path: asset_root.clone(),
        source,
    })?;
    validate_asset_ancestry(repository_root, &docs, &assets, &asset_root)?;
    let asset_paths = [
        asset_root.join(format!("{directory_version}.svg")),
        asset_root.join(format!("{directory_version}.png")),
        asset_root.join(format!("{directory_version}-story.json")),
    ];
    let mut completed_assets = Vec::with_capacity(asset_paths.len());
    for (index, (path, bytes)) in asset_paths
        .iter()
        .zip([&preview.svg, &preview.png, &preview.manifest])
        .enumerate()
    {
        if let Err(source) = validate_asset_ancestry(repository_root, &docs, &assets, &asset_root) {
            return Err(AcceptanceError::publication(completed_assets, source));
        }
        match write_immutable_asset(path, bytes) {
            Ok(AssetPublication::Created) => completed_assets.push(path.clone()),
            Ok(AssetPublication::Existing) if !previously_published[index] => {
                completed_assets.push(path.clone());
            }
            Ok(AssetPublication::Existing) => {}
            Err(source) => {
                return Err(AcceptanceError::publication(completed_assets, source));
            }
        }
    }
    if let Err(source) =
        crate::preview_operation::reject_reparse_point(repository_root, "repository root")
            .and_then(|()| crate::preview_operation::reject_reparse_point(&readme, "README"))
    {
        return Err(AcceptanceError::publication(
            completed_assets,
            source.into(),
        ));
    }
    if let Err(source) = replace_file_atomically(&readme, replacement.as_bytes()) {
        return Err(AcceptanceError::publication(completed_assets, source));
    }

    Ok(AcceptanceReceipt {
        assets: asset_paths,
        readme,
        digest: actual,
    })
}

fn validate_asset_ancestry(
    repository_root: &Path,
    docs: &Path,
    assets: &Path,
    asset_root: &Path,
) -> Result<(), AcceptanceError> {
    for (path, stage) in [
        (repository_root, "repository root"),
        (docs, "docs directory"),
        (assets, "assets directory"),
        (asset_root, "showcase asset directory"),
    ] {
        crate::preview_operation::reject_reparse_point(path, stage)?;
    }
    Ok(())
}

fn replace_showcase_section(readme: &str, story: &ReleaseStory) -> Result<String, AcceptanceError> {
    let visible = story.visible_issue_numbers.len();
    let resolved = story
        .final_statuses
        .iter()
        .filter(|status| status.status == Some(Status::Resolved))
        .count();
    let version = &story.release_version;
    let version_url = encode_url_component(version);
    let version_html = escape_html(version);
    let version_markdown = escape_markdown_text(version);
    let section = format!(
        "{README_START}\n## Release constellation\n\n<picture>\n  <source media=\"(prefers-reduced-motion: reduce)\" srcset=\"docs/assets/readme-showcase/{version_url}.png\">\n  <img src=\"docs/assets/readme-showcase/{version_url}.svg\" alt=\"Stellr {version_html} release constellation: {visible} issues, {resolved} resolved\">\n</picture>\n\n[View the static {version_markdown} release constellation](docs/assets/readme-showcase/{version_url}.png).\n\nRelease {version_markdown} charts {visible} visible issues, with {resolved} resolved at the recorded cutoff.\n{README_END}"
    );

    if let Some(start) = readme.find(README_START) {
        let relative_end = readme[start..].find(README_END).ok_or_else(|| {
            AcceptanceError::InvalidReadme("start marker has no end marker".into())
        })?;
        let end = start + relative_end + README_END.len();
        return Ok(format!("{}{}{}", &readme[..start], section, &readme[end..]));
    }

    let start = readme.find(LEGACY_HEADING).ok_or_else(|| {
        AcceptanceError::InvalidReadme("no showcase markers or compatibility heading".into())
    })?;
    let following = &readme[start + LEGACY_HEADING.len()..];
    let relative_end = following.find("\n## ").ok_or_else(|| {
        AcceptanceError::InvalidReadme("compatibility section has no following heading".into())
    })?;
    let end = start + LEGACY_HEADING.len() + relative_end + 1;
    Ok(format!(
        "{}{}\n\n{}",
        &readme[..start],
        section,
        &readme[end..]
    ))
}

fn encode_url_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            use std::fmt::Write as _;
            write!(&mut encoded, "%{byte:02X}").expect("writing to a string cannot fail");
        }
    }
    encoded
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_markdown_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_ascii_punctuation() {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

fn write_immutable_asset(path: &Path, bytes: &[u8]) -> Result<AssetPublication, AcceptanceError> {
    crate::preview_operation::reject_reparse_point(path, "versioned showcase asset")?;
    if path.exists() {
        let existing = fs::read(path).map_err(|source| AcceptanceError::Filesystem {
            stage: "read versioned showcase asset",
            path: path.to_path_buf(),
            source,
        })?;
        return if existing == bytes {
            Ok(AssetPublication::Existing)
        } else {
            Err(AcceptanceError::AssetConflict(path.to_path_buf()))
        };
    }
    let temporary = sibling_path(path, "tmp");
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|source| AcceptanceError::Filesystem {
            stage: "create temporary showcase asset",
            path: temporary.clone(),
            source,
        })?;
    if let Err(source) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(&temporary);
        return Err(AcceptanceError::Filesystem {
            stage: "write temporary showcase asset",
            path: temporary,
            source,
        });
    }
    drop(file);
    if let Err(source) = rename_without_replace(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        if source.kind() == io::ErrorKind::AlreadyExists {
            return match fs::read(path) {
                Ok(existing) if existing == bytes => Ok(AssetPublication::Existing),
                Ok(_) => Err(AcceptanceError::AssetConflict(path.to_path_buf())),
                Err(read_source) => Err(AcceptanceError::Filesystem {
                    stage: "inspect concurrent versioned showcase asset",
                    path: path.to_path_buf(),
                    source: read_source,
                }),
            };
        }
        return Err(AcceptanceError::Filesystem {
            stage: "publish versioned showcase asset",
            path: path.to_path_buf(),
            source,
        });
    }
    let stored = fs::read(path).map_err(|source| AcceptanceError::Filesystem {
        stage: "reverify versioned showcase asset",
        path: path.to_path_buf(),
        source,
    })?;
    if stored != bytes {
        return Err(AcceptanceError::InvalidPreview(format!(
            "published asset bytes differ at {}",
            path.display()
        )));
    }
    Ok(AssetPublication::Created)
}

#[cfg(windows)]
fn rename_without_replace(temporary: &Path, target: &Path) -> io::Result<()> {
    use std::{iter, os::windows::ffi::OsStrExt};
    use windows_sys::Win32::Storage::FileSystem::MoveFileExW;

    let wide = |path: &Path| {
        path.as_os_str()
            .encode_wide()
            .chain(iter::once(0))
            .collect::<Vec<_>>()
    };
    let temporary = wide(temporary);
    let target = wide(target);
    let moved = unsafe { MoveFileExW(temporary.as_ptr(), target.as_ptr(), 0) };
    if moved == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn rename_without_replace(temporary: &Path, target: &Path) -> io::Result<()> {
    fs::hard_link(temporary, target)?;
    fs::remove_file(temporary)
}

fn sibling_path(path: &Path, suffix: &str) -> PathBuf {
    let name = path
        .file_name()
        .expect("publication targets always have file names")
        .to_string_lossy();
    path.with_file_name(format!(
        ".{name}.{}.{}.{}",
        std::process::id(),
        NEXT_ACCEPTANCE_ID.fetch_add(1, Ordering::Relaxed),
        suffix
    ))
}

fn replace_file_atomically(path: &Path, bytes: &[u8]) -> Result<(), AcceptanceError> {
    let temporary = sibling_path(path, "tmp");
    let backup = sibling_path(path, "backup");
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|source| AcceptanceError::Filesystem {
            stage: "create temporary README",
            path: temporary.clone(),
            source,
        })?;
    if let Err(source) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(&temporary);
        return Err(AcceptanceError::Filesystem {
            stage: "write temporary README",
            path: temporary,
            source,
        });
    }
    drop(file);

    let result = replace_existing_file(&temporary, path, &backup);
    finish_readme_replacement(&temporary, path, &backup, result).map_err(|source| {
        AcceptanceError::Filesystem {
            stage: "replace README",
            path: path.to_path_buf(),
            source,
        }
    })?;
    verify_replaced_readme(path, &backup, bytes).map_err(|source| AcceptanceError::Filesystem {
        stage: "reverify README",
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn verify_replaced_readme(target: &Path, backup: &Path, expected: &[u8]) -> io::Result<()> {
    verify_replaced_readme_with(
        target,
        backup,
        expected,
        |path| fs::read(path),
        restore_or_move_readme,
    )
}

fn verify_replaced_readme_with<Read, Restore>(
    target: &Path,
    backup: &Path,
    expected: &[u8],
    read: Read,
    restore: Restore,
) -> io::Result<()>
where
    Read: FnOnce(&Path) -> io::Result<Vec<u8>>,
    Restore: FnOnce(&Path, &Path) -> io::Result<()>,
{
    match read(target) {
        Ok(stored) if stored == expected => {
            if backup.try_exists()? {
                let _ = fs::remove_file(backup);
            }
            Ok(())
        }
        Ok(_) => {
            if backup.try_exists()? {
                restore(backup, target)?;
            }
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "README bytes changed during atomic replacement",
            ))
        }
        Err(error) => {
            if backup.try_exists()? {
                restore(backup, target)?;
            }
            Err(error)
        }
    }
}

fn finish_readme_replacement(
    temporary: &Path,
    target: &Path,
    backup: &Path,
    replacement_result: io::Result<()>,
) -> io::Result<()> {
    finish_readme_replacement_with(
        temporary,
        target,
        backup,
        replacement_result,
        restore_or_move_readme,
    )
}

fn finish_readme_replacement_with<R>(
    temporary: &Path,
    target: &Path,
    backup: &Path,
    replacement_result: io::Result<()>,
    mut rename: R,
) -> io::Result<()>
where
    R: FnMut(&Path, &Path) -> io::Result<()>,
{
    match replacement_result {
        Ok(()) if target.try_exists()? => {
            if temporary.try_exists()? {
                fs::remove_file(temporary)?;
            }
            Ok(())
        }
        Ok(()) if temporary.try_exists()? => {
            rename(temporary, target)?;
            Ok(())
        }
        Ok(()) if backup.try_exists()? => {
            rename(backup, target)?;
            Err(io::Error::other(
                "README replacement reported success without the replacement file",
            ))
        }
        Ok(()) => Err(io::Error::other(
            "README replacement reported success without a recoverable file",
        )),
        Err(error) if backup.try_exists()? => {
            rename(backup, target)?;
            if temporary.try_exists()? {
                fs::remove_file(temporary)?;
            }
            Err(error)
        }
        Err(error) if target.try_exists()? => {
            if temporary.try_exists()? {
                fs::remove_file(temporary)?;
            }
            Err(error)
        }
        Err(error) => Err(error),
    }
}

#[cfg(windows)]
fn restore_or_move_readme(source: &Path, target: &Path) -> io::Result<()> {
    if !target.try_exists()? {
        return fs::rename(source, target);
    }

    use std::{iter, os::windows::ffi::OsStrExt};
    use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

    let wide = |path: &Path| {
        path.as_os_str()
            .encode_wide()
            .chain(iter::once(0))
            .collect::<Vec<_>>()
    };
    let target = wide(target);
    let source = wide(source);
    let replaced = unsafe {
        ReplaceFileW(
            target.as_ptr(),
            source.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if replaced == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn restore_or_move_readme(source: &Path, target: &Path) -> io::Result<()> {
    fs::rename(source, target)
}

#[cfg(windows)]
fn replace_existing_file(temporary: &Path, target: &Path, backup: &Path) -> io::Result<()> {
    use std::{iter, os::windows::ffi::OsStrExt};
    use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

    let wide = |path: &Path| {
        path.as_os_str()
            .encode_wide()
            .chain(iter::once(0))
            .collect::<Vec<_>>()
    };
    let target = wide(target);
    let temporary = wide(temporary);
    let backup = wide(backup);
    let replaced = unsafe {
        ReplaceFileW(
            target.as_ptr(),
            temporary.as_ptr(),
            backup.as_ptr(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if replaced == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_existing_file(temporary: &Path, target: &Path, _backup: &Path) -> io::Result<()> {
    fs::rename(temporary, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_readme_replacement_restores_the_displaced_backup() {
        let root = std::env::temp_dir().join(format!(
            "stellr-readme-recovery-{}-{}",
            std::process::id(),
            NEXT_ACCEPTANCE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let temporary = root.join("README.tmp");
        let target = root.join("README.md");
        let backup = root.join("README.backup");
        fs::write(&temporary, b"new README").unwrap();
        fs::write(&backup, b"old README").unwrap();
        let replacement_error = io::Error::new(io::ErrorKind::PermissionDenied, "replace failed");

        let error = finish_readme_replacement_with(
            &temporary,
            &target,
            &backup,
            Err(replacement_error),
            |from, to| fs::rename(from, to),
        )
        .unwrap_err();

        assert_eq!(error.to_string(), "replace failed");
        assert_eq!(fs::read(&target).unwrap(), b"old README");
        assert!(!temporary.exists());
        assert!(!backup.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_readme_replacement_restores_backup_over_a_changed_target() {
        let root = std::env::temp_dir().join(format!(
            "stellr-readme-changed-target-{}-{}",
            std::process::id(),
            NEXT_ACCEPTANCE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let temporary = root.join("README.tmp");
        let target = root.join("README.md");
        let backup = root.join("README.backup");
        fs::write(&temporary, b"new temporary README").unwrap();
        fs::write(&target, b"potentially changed README").unwrap();
        fs::write(&backup, b"old README").unwrap();
        let replacement_error = io::Error::new(io::ErrorKind::PermissionDenied, "replace failed");

        let error = finish_readme_replacement_with(
            &temporary,
            &target,
            &backup,
            Err(replacement_error),
            |from, to| {
                if to.exists() {
                    fs::remove_file(to)?;
                }
                fs::rename(from, to)
            },
        )
        .unwrap_err();

        assert_eq!(error.to_string(), "replace failed");
        assert_eq!(fs::read(&target).unwrap(), b"old README");
        assert!(!temporary.exists());
        assert!(!backup.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn successful_replacement_restores_backup_when_verification_mismatches() {
        let root = std::env::temp_dir().join(format!(
            "stellr-readme-verification-mismatch-{}-{}",
            std::process::id(),
            NEXT_ACCEPTANCE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let target = root.join("README.md");
        let backup = root.join("README.backup");
        fs::write(&target, b"new but unverified README").unwrap();
        fs::write(&backup, b"old README").unwrap();

        let error = verify_replaced_readme_with(
            &target,
            &backup,
            b"expected new README",
            |_| Ok(b"mismatched bytes".to_vec()),
            |source, destination| {
                if destination.exists() {
                    fs::remove_file(destination)?;
                }
                fs::rename(source, destination)
            },
        )
        .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(fs::read(&target).unwrap(), b"old README");
        assert!(!backup.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
