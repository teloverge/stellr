use std::{fs, io, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedRoute {
    pub space: String,
    pub issue: Option<u64>,
}

impl PersistedRoute {
    pub fn new(space: impl Into<String>, issue: Option<u64>) -> Option<Self> {
        let space = space.into();
        if space.trim().is_empty() || matches!(issue, Some(0)) {
            return None;
        }
        Some(Self { space, issue })
    }

    fn valid(self) -> Option<Self> {
        Self::new(self.space, self.issue)
    }
}

#[derive(Clone)]
pub struct RouteStateStore {
    file: PathBuf,
}

impl RouteStateStore {
    pub fn new(file: PathBuf) -> Self {
        Self { file }
    }

    pub fn default_file() -> PathBuf {
        stellr_server::spaces::SpaceStore::default_file().with_file_name("desktop-route.json")
    }

    pub fn load(&self) -> Option<PersistedRoute> {
        let bytes = fs::read(&self.file).ok()?;
        serde_json::from_slice::<PersistedRoute>(&bytes)
            .ok()?
            .valid()
    }

    pub fn save(&self, route: Option<PersistedRoute>) -> io::Result<()> {
        let Some(route) = route else {
            match fs::remove_file(&self.file) {
                Ok(()) => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(error),
            }
        };
        let Some(parent) = self.file.parent() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "route-state file has no parent directory",
            ));
        };
        fs::create_dir_all(parent)?;
        fs::write(&self.file, serde_json::to_vec(&route)?)
    }
}
