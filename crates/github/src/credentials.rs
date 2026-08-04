//! Operating-system credential storage for GitHub authorization.

pub const SERVICE_NAME: &str = "stellr.github";
pub const DEFAULT_ACCOUNT: &str = "default";

#[derive(Debug, thiserror::Error)]
pub enum CredentialStoreError {
    #[error("operating-system credential store failed: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("operating-system credential store failed: {0}")]
    Message(String),
}

pub trait CredentialStore: Send + Sync {
    fn load(&self) -> Result<Option<String>, CredentialStoreError>;
    fn store(&self, credential: &str) -> Result<(), CredentialStoreError>;
}

#[derive(Debug, Clone)]
pub struct OsCredentialStore {
    service: &'static str,
    account: &'static str,
}

impl Default for OsCredentialStore {
    fn default() -> Self {
        Self {
            service: SERVICE_NAME,
            account: DEFAULT_ACCOUNT,
        }
    }
}

impl OsCredentialStore {
    pub fn service(&self) -> &str {
        self.service
    }

    pub fn account(&self) -> &str {
        self.account
    }

    fn entry(&self) -> Result<keyring::Entry, CredentialStoreError> {
        keyring::Entry::new(self.service, self.account).map_err(Into::into)
    }
}

impl CredentialStore for OsCredentialStore {
    fn load(&self) -> Result<Option<String>, CredentialStoreError> {
        match self.entry()?.get_password() {
            Ok(credential) => Ok(Some(credential)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn store(&self, credential: &str) -> Result<(), CredentialStoreError> {
        if credential.trim().is_empty() {
            return Err(CredentialStoreError::Message(
                "refusing to store a blank credential".to_owned(),
            ));
        }
        self.entry()?.set_password(credential).map_err(Into::into)
    }
}
