use std::path::PathBuf;

use crate::error::AppResult;
use crate::models::HostRecord;

use super::crypto::EncryptedJsonStore;

#[derive(Debug, Clone)]
pub struct CredentialsStore {
    vault: EncryptedJsonStore,
}

impl CredentialsStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            vault: EncryptedJsonStore::new(path),
        }
    }

    pub fn with_key(path: PathBuf, key: [u8; 32]) -> Self {
        Self {
            vault: EncryptedJsonStore::with_key(path, key),
        }
    }

    pub fn load_hosts(&self) -> AppResult<Vec<HostRecord>> {
        self.vault.load_or_default()
    }

    pub fn save_hosts(&self, hosts: &[HostRecord]) -> AppResult<()> {
        self.vault.save(&hosts)
    }
}
