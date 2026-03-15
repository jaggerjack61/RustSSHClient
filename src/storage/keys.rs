use std::path::PathBuf;

use crate::error::AppResult;
use crate::models::SshKeyRecord;

use super::crypto::EncryptedJsonStore;

#[derive(Debug, Clone)]
pub struct KeyStore {
    vault: EncryptedJsonStore,
}

impl KeyStore {
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

    pub fn load_keys(&self) -> AppResult<Vec<SshKeyRecord>> {
        self.vault.load_or_default()
    }

    pub fn save_keys(&self, keys: &[SshKeyRecord]) -> AppResult<()> {
        self.vault.save(&keys)
    }
}
