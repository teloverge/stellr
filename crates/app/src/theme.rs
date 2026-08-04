use std::{fs, io, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone)]
pub struct ThemeStore {
    file: PathBuf,
}

impl ThemeStore {
    pub fn new(file: PathBuf) -> Self {
        Self { file }
    }

    pub fn default_file() -> PathBuf {
        stellr_server::spaces::SpaceStore::default_file().with_file_name("theme.json")
    }

    pub fn load(&self) -> ThemePreference {
        fs::read(&self.file)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, preference: ThemePreference) -> io::Result<()> {
        let Some(parent) = self.file.parent() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "theme-preference file has no parent directory",
            ));
        };
        fs::create_dir_all(parent)?;
        fs::write(&self.file, serde_json::to_vec(&preference)?)
    }
}
