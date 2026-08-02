use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use stellr_core::RepoRef;

static NEXT_SPACE_FILE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceEntry {
    pub id: String,
    pub path: Option<PathBuf>,
    pub repo: RepoRef,
}

impl SpaceEntry {
    pub fn new(repo: RepoRef, path: Option<PathBuf>) -> Self {
        Self {
            id: format!("{}-{}", repo.owner, repo.name),
            path,
            repo,
        }
    }
}

pub struct SpaceStore {
    entries: Vec<SpaceEntry>,
    file: PathBuf,
}

impl SpaceStore {
    pub fn default_file() -> PathBuf {
        ProjectDirs::from("", "", "stellr")
            .map(|directories| directories.config_dir().join("spaces.toml"))
            .unwrap_or_else(|| PathBuf::from("spaces.toml"))
    }

    pub fn load(file: PathBuf) -> Self {
        let entries = fs::read_to_string(&file)
            .ok()
            .and_then(|contents| toml::from_str::<StoredSpaces>(&contents).ok())
            .map(|stored| stored.spaces.into_iter().map(SpaceEntry::from).collect())
            .unwrap_or_default();
        Self { entries, file }
    }

    pub fn entries(&self) -> &[SpaceEntry] {
        &self.entries
    }

    pub fn add(&mut self, entry: SpaceEntry) -> Result<(), String> {
        if self.entries.iter().any(|existing| existing.id == entry.id) {
            return Err(format!("space {} already exists", entry.id));
        }
        self.entries.push(entry);
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let Some(index) = self.entries.iter().position(|entry| entry.id == id) else {
            return false;
        };
        self.entries.remove(index);
        true
    }

    pub fn save(&self) -> io::Result<()> {
        if let Some(directory) = self.file.parent() {
            fs::create_dir_all(directory)?;
        }
        let stored = StoredSpaces {
            spaces: self.entries.iter().map(StoredSpace::from).collect(),
        };
        let contents = toml::to_string_pretty(&stored).map_err(io::Error::other)?;
        write_atomic(&self.file, contents.as_bytes())
    }
}

fn write_atomic(target: &Path, contents: &[u8]) -> io::Result<()> {
    let directory = target
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(directory)?;
    let file_name = target.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "spaces file has no file name")
    })?;

    for _ in 0..16 {
        let temporary = directory.join(format!(
            ".{}.{}.{}.tmp",
            file_name.to_string_lossy(),
            std::process::id(),
            NEXT_SPACE_FILE_ID.fetch_add(1, Ordering::Relaxed)
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
        let write_result = file.write_all(contents).and_then(|()| file.sync_all());
        drop(file);
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        if let Err(error) = replace_file(&temporary, target) {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique spaces temporary file",
    ))
}

fn replace_file(temporary: &Path, target: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        if target.try_exists()? {
            replace_file_windows(temporary, target)
        } else {
            match fs::rename(temporary, target) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    replace_file_windows(temporary, target)
                }
                Err(error) => Err(error),
            }
        }
    }

    #[cfg(not(windows))]
    {
        fs::rename(temporary, target)
    }
}

#[cfg(windows)]
fn replace_file_windows(temporary: &Path, target: &Path) -> io::Result<()> {
    use std::{iter, os::windows::ffi::OsStrExt};
    use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

    let target = target
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect::<Vec<_>>();
    let temporary = temporary
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect::<Vec<_>>();
    let replaced = unsafe {
        ReplaceFileW(
            target.as_ptr(),
            temporary.as_ptr(),
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

#[derive(Serialize, Deserialize)]
struct StoredSpaces {
    #[serde(default)]
    spaces: Vec<StoredSpace>,
}

#[derive(Serialize, Deserialize)]
struct StoredSpace {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<PathBuf>,
    owner: String,
    name: String,
}

impl From<StoredSpace> for SpaceEntry {
    fn from(stored: StoredSpace) -> Self {
        Self {
            id: stored.id,
            path: stored.path,
            repo: RepoRef {
                owner: stored.owner,
                name: stored.name,
            },
        }
    }
}

impl From<&SpaceEntry> for StoredSpace {
    fn from(entry: &SpaceEntry) -> Self {
        Self {
            id: entry.id.clone(),
            path: entry.path.clone(),
            owner: entry.repo.owner.clone(),
            name: entry.repo.name.clone(),
        }
    }
}

pub fn parse_remote(url: &str) -> Option<RepoRef> {
    let path = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("git@github.com:"))
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))?
        .trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut parts = path.split('/');
    let (Some(owner), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return None;
    };
    if owner.is_empty() || name.is_empty() {
        return None;
    }

    Some(RepoRef {
        owner: owner.into(),
        name: name.into(),
    })
}

pub fn detect_repo(path: &Path) -> Result<RepoRef, String> {
    if !path.is_dir() {
        return Err("not a git repo".into());
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["remote", "get-url", "origin"])
        .output()
        .map_err(|_| "not a git repo".to_string())?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return if error.contains("not a git repository") {
            Err("not a git repo".into())
        } else {
            Err("origin is not a GitHub remote".into())
        };
    }

    parse_remote(String::from_utf8_lossy(&output.stdout).trim())
        .ok_or_else(|| "origin is not a GitHub remote".into())
}

#[cfg(test)]
mod tests {
    use super::{SpaceEntry, SpaceStore, detect_repo, parse_remote};
    use std::path::PathBuf;
    use stellr_core::RepoRef;

    #[test]
    fn parses_only_supported_github_remote_forms() {
        for url in [
            "https://github.com/o/r",
            "https://github.com/o/r.git",
            "git@github.com:o/r.git",
            "ssh://git@github.com/o/r",
        ] {
            let repo = parse_remote(url).unwrap();
            assert_eq!(
                (repo.owner.as_str(), repo.name.as_str()),
                ("o", "r"),
                "{url}"
            );
        }

        assert!(parse_remote("https://gitlab.com/o/r").is_none());
    }

    #[test]
    fn missing_path_is_reported_as_not_a_git_repo() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("missing");

        assert_eq!(detect_repo(&missing), Err("not a git repo".into()));
    }

    #[test]
    fn space_store_persists_added_entries() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("spaces.toml");
        let entry = SpaceEntry {
            id: "o-r".into(),
            path: Some(PathBuf::from(r"C:\dev\r")),
            repo: RepoRef {
                owner: "o".into(),
                name: "r".into(),
            },
        };

        let mut store = SpaceStore::load(file.clone());
        store.add(entry.clone()).unwrap();
        store.save().unwrap();

        let reloaded = SpaceStore::load(file);
        assert_eq!(reloaded.entries(), &[entry]);
    }

    #[test]
    fn space_store_persists_removal() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("spaces.toml");
        let entry = SpaceEntry {
            id: "o-r".into(),
            path: None,
            repo: RepoRef {
                owner: "o".into(),
                name: "r".into(),
            },
        };
        let mut store = SpaceStore::load(file.clone());
        store.add(entry).unwrap();
        store.save().unwrap();

        assert!(store.remove("o-r"));
        store.save().unwrap();

        assert!(SpaceStore::load(file).entries().is_empty());
    }
}
