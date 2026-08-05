use std::{
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use stellr_core::{RawIssue, RepoRef};

static NEXT_ARTIFACT_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub issues: Vec<RawIssue>,
    pub synced_at: i64,
}

pub struct Cache {
    root: PathBuf,
}

impl Cache {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn default_root() -> PathBuf {
        ProjectDirs::from("", "", "stellr")
            .map(|dirs| dirs.data_dir().join("cache"))
            .unwrap_or_else(|| PathBuf::from("cache"))
    }

    pub fn load(&self, repo: &RepoRef) -> Option<Snapshot> {
        let bytes = fs::read(self.path_for(repo)).ok()?;
        let mut snapshot: Snapshot = serde_json::from_slice(&bytes).ok()?;
        crate::textref::enrich_relationships(&mut snapshot.issues);
        Some(snapshot)
    }

    /// Writes a snapshot through a same-directory temporary file.
    ///
    /// On Windows, replacing an existing cache file uses `ReplaceFileW` with
    /// an atomic replacement. This avoids the missing-file interval caused by
    /// a remove-then-rename sequence. A unique same-directory backup path
    /// keeps the displaced snapshot recoverable across partial failures.
    pub fn store(&self, repo: &RepoRef, snapshot: &Snapshot) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        let target = self.path_for(repo);
        let backup = allocate_backup_path(&target)?;
        let temporary = write_temporary_snapshot(&self.root, repo, snapshot)?;
        let result = replace_snapshot(&temporary, &target, &backup);
        finish_replacement(&temporary, &target, &backup, result)
    }

    fn path_for(&self, repo: &RepoRef) -> PathBuf {
        self.root
            .join(format!("{}__{}.json", repo.owner, repo.name))
    }
}

fn write_temporary_snapshot(
    root: &Path,
    repo: &RepoRef,
    snapshot: &Snapshot,
) -> io::Result<PathBuf> {
    for _ in 0..16 {
        let temporary = root.join(format!(
            ".{}__{}.json.{}.{}.tmp",
            repo.owner,
            repo.name,
            std::process::id(),
            NEXT_ARTIFACT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };

        if let Err(error) = serde_json::to_writer(&mut file, snapshot)
            .map_err(io::Error::other)
            .and_then(|()| file.sync_all())
        {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }

        return Ok(temporary);
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique snapshot temporary file",
    ))
}

fn allocate_backup_path(target: &Path) -> io::Result<PathBuf> {
    let directory = target.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "snapshot target has no parent directory",
        )
    })?;
    let file_name = target.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "snapshot target has no file name",
        )
    })?;

    for _ in 0..16 {
        let backup = directory.join(format!(
            ".{}.{}.{}.backup",
            file_name.to_string_lossy(),
            std::process::id(),
            NEXT_ARTIFACT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        if !backup.try_exists()? {
            return Ok(backup);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique snapshot backup path",
    ))
}

fn replace_snapshot(temporary: &Path, target: &Path, backup: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        if target.try_exists()? {
            replace_file_windows(temporary, target, backup)
        } else {
            match fs::rename(temporary, target) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    replace_file_windows(temporary, target, backup)
                }
                Err(error) => Err(error),
            }
        }
    }

    #[cfg(not(windows))]
    {
        let _ = backup;
        fs::rename(temporary, target)
    }
}

#[cfg(windows)]
fn replace_file_windows(temporary: &Path, target: &Path, backup: &Path) -> io::Result<()> {
    use std::{iter, os::windows::ffi::OsStrExt};
    use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

    let target_wide = target
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect::<Vec<_>>();
    let temporary_wide = temporary
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect::<Vec<_>>();
    let backup_wide = backup
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect::<Vec<_>>();

    // Both paths are in the cache directory, so ReplaceFileW keeps the
    // replacement on one volume and atomically swaps an existing destination.
    let replaced = unsafe {
        ReplaceFileW(
            target_wide.as_ptr(),
            temporary_wide.as_ptr(),
            backup_wide.as_ptr(),
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

fn finish_replacement(
    temporary: &Path,
    target: &Path,
    backup: &Path,
    replacement_result: io::Result<()>,
) -> io::Result<()> {
    finish_replacement_with(temporary, target, backup, replacement_result, |from, to| {
        fs::rename(from, to)
    })
}

fn finish_replacement_with<R>(
    temporary: &Path,
    target: &Path,
    backup: &Path,
    replacement_result: io::Result<()>,
    rename: R,
) -> io::Result<()>
where
    R: FnOnce(&Path, &Path) -> io::Result<()>,
{
    if target.try_exists()? {
        if temporary.try_exists()? {
            fs::remove_file(temporary)?;
        }
        if backup.try_exists()? {
            fs::remove_file(backup)?;
        }
        replacement_result
    } else if temporary.try_exists()? {
        rename(temporary, target)?;
        if backup.try_exists()? {
            fs::remove_file(backup)?;
        }
        Ok(())
    } else if backup.try_exists()? {
        rename(backup, target)?;
        replacement_result
    } else {
        replacement_result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellr_core::{IssueState, RawIssue, RepoRef};

    fn repo() -> RepoRef {
        RepoRef {
            owner: "o".into(),
            name: "r".into(),
        }
    }

    fn snapshot(title: &str, synced_at: i64) -> Snapshot {
        Snapshot {
            issues: vec![RawIssue {
                number: 1,
                parent_issue: None,
                title: title.into(),
                body: String::new(),
                state: IssueState::Open,
                assignees: vec![],
                milestone: None,
                labels: vec![],
                blocked_by: vec![],
                url: "u".into(),
            }],
            synced_at,
        }
    }

    #[test]
    fn store_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().to_path_buf());
        let repo = repo();
        assert!(cache.load(&repo).is_none());

        let snapshot = snapshot("t", 1_753_000_000);
        cache.store(&repo, &snapshot).unwrap();

        let loaded = cache.load(&repo).unwrap();
        assert_eq!(loaded, snapshot);
    }

    #[test]
    fn corrupt_file_loads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().to_path_buf());
        let repo = repo();
        std::fs::write(dir.path().join("o__r.json"), b"{not json").unwrap();

        assert!(cache.load(&repo).is_none());
    }

    #[test]
    fn load_accepts_older_snapshot_without_parent_issue() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().to_path_buf());
        let repo = repo();
        std::fs::write(
            dir.path().join("o__r.json"),
            br#"{
                "issues": [{
                    "number": 1,
                    "title": "Older snapshot",
                    "body": "",
                    "state": "open",
                    "assignees": [],
                    "milestone": null,
                    "labels": [],
                    "blocked_by": [],
                    "url": "u"
                }],
                "synced_at": 1753000000
            }"#,
        )
        .unwrap();

        let snapshot = cache.load(&repo).unwrap();

        assert_eq!(snapshot.issues[0].parent_issue, None);
    }

    #[test]
    fn load_enriches_relationships_from_cached_bodies() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().to_path_buf());
        let repo = repo();
        let mut source = snapshot("Root", 1_753_000_000);
        source.issues.push(RawIssue {
            number: 2,
            parent_issue: None,
            title: "Dependent".into(),
            body: "## Parent\n\n#1\n## Blocked by\n\n- #1".into(),
            state: IssueState::Open,
            assignees: vec![],
            milestone: None,
            labels: vec![],
            blocked_by: vec![],
            url: "u2".into(),
        });
        source.issues.push(RawIssue {
            number: 3,
            parent_issue: None,
            title: "Blocker by inversion".into(),
            body: "## Blocks\n\n- #2".into(),
            state: IssueState::Open,
            assignees: vec![],
            milestone: None,
            labels: vec![],
            blocked_by: vec![],
            url: "u3".into(),
        });

        cache.store(&repo, &source).unwrap();
        let loaded = cache.load(&repo).unwrap();

        assert_eq!(loaded.issues[1].parent_issue, Some(1));
        assert_eq!(loaded.issues[1].blocked_by, vec![1, 3]);

        cache.store(&repo, &loaded).unwrap();
        let loaded_again = cache.load(&repo).unwrap();

        assert_eq!(loaded_again, loaded);
    }

    #[test]
    fn store_replaces_an_existing_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().to_path_buf());
        let repo = repo();

        cache.store(&repo, &snapshot("old", 1)).unwrap();
        cache.store(&repo, &snapshot("new", 2)).unwrap();

        assert_eq!(cache.load(&repo).unwrap(), snapshot("new", 2));
        let files = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(files, [std::ffi::OsString::from("o__r.json")]);
    }

    #[test]
    fn backup_paths_are_unique_siblings_of_the_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("o__r.json");

        let first = allocate_backup_path(&target).unwrap();
        let second = allocate_backup_path(&target).unwrap();

        assert_eq!(first.parent(), Some(dir.path()));
        assert_eq!(second.parent(), Some(dir.path()));
        assert_ne!(first, second);
        assert!(!first.exists());
        assert!(!second.exists());
    }

    #[test]
    fn replacement_failure_promotes_temp_and_removes_backup() {
        let dir = tempfile::tempdir().unwrap();
        let temporary = dir.path().join("snapshot.tmp");
        let target = dir.path().join("snapshot.json");
        let backup = dir.path().join("snapshot.backup");
        std::fs::write(&temporary, b"new snapshot").unwrap();
        std::fs::write(&backup, b"old snapshot").unwrap();

        finish_replacement_with(
            &temporary,
            &target,
            &backup,
            Err(io::Error::other("replace failed")),
            |from, to| fs::rename(from, to),
        )
        .unwrap();

        assert_eq!(std::fs::read(&target).unwrap(), b"new snapshot");
        assert!(!temporary.exists());
        assert!(!backup.exists());
    }

    #[test]
    fn replacement_failure_cleans_temp_and_backup_when_target_exists() {
        let dir = tempfile::tempdir().unwrap();
        let temporary = dir.path().join("snapshot.tmp");
        let target = dir.path().join("snapshot.json");
        let backup = dir.path().join("snapshot.backup");
        std::fs::write(&temporary, b"new snapshot").unwrap();
        std::fs::write(&target, b"old snapshot").unwrap();
        std::fs::write(&backup, b"older snapshot").unwrap();

        let error = finish_replacement_with(
            &temporary,
            &target,
            &backup,
            Err(io::Error::other("replace failed")),
            |_, _| panic!("recovery rename must not run when the target exists"),
        )
        .unwrap_err();

        assert_eq!(error.to_string(), "replace failed");
        assert_eq!(std::fs::read(&target).unwrap(), b"old snapshot");
        assert!(!temporary.exists());
        assert!(!backup.exists());
    }

    #[test]
    fn replacement_failure_preserves_temp_and_backup_when_recovery_fails() {
        let dir = tempfile::tempdir().unwrap();
        let temporary = dir.path().join("snapshot.tmp");
        let target = dir.path().join("snapshot.json");
        let backup = dir.path().join("snapshot.backup");
        std::fs::write(&temporary, b"new snapshot").unwrap();
        std::fs::write(&backup, b"old snapshot").unwrap();

        let error = finish_replacement_with(
            &temporary,
            &target,
            &backup,
            Err(io::Error::other("replace failed")),
            |_, _| {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "rename failed",
                ))
            },
        )
        .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(error.to_string(), "rename failed");
        assert!(temporary.exists());
        assert!(backup.exists());
        assert!(!target.exists());
    }

    #[test]
    fn replacement_failure_restores_backup_when_temp_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let temporary = dir.path().join("snapshot.tmp");
        let target = dir.path().join("snapshot.json");
        let backup = dir.path().join("snapshot.backup");
        std::fs::write(&backup, b"old snapshot").unwrap();

        let error = finish_replacement_with(
            &temporary,
            &target,
            &backup,
            Err(io::Error::other("replace failed")),
            |from, to| fs::rename(from, to),
        )
        .unwrap_err();

        assert_eq!(error.to_string(), "replace failed");
        assert_eq!(std::fs::read(&target).unwrap(), b"old snapshot");
        assert!(!temporary.exists());
        assert!(!backup.exists());
    }

    #[test]
    fn replacement_success_removes_backup_when_target_exists() {
        let dir = tempfile::tempdir().unwrap();
        let temporary = dir.path().join("snapshot.tmp");
        let target = dir.path().join("snapshot.json");
        let backup = dir.path().join("snapshot.backup");
        std::fs::write(&target, b"new snapshot").unwrap();
        std::fs::write(&backup, b"old snapshot").unwrap();

        finish_replacement_with(&temporary, &target, &backup, Ok(()), |_, _| {
            panic!("recovery rename must not run when the target exists")
        })
        .unwrap();

        assert_eq!(std::fs::read(&target).unwrap(), b"new snapshot");
        assert!(!backup.exists());
    }
}
