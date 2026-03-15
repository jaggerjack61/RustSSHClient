pub mod credentials;
pub mod crypto;
pub mod keys;

use std::path::PathBuf;

use chrono::Utc;
use directories::ProjectDirs;

use crate::error::{AppError, AppResult};
use crate::models::{HostRecord, SshKeyRecord};

pub const APP_QUALIFIER: &str = "com";
pub const APP_ORGANIZATION: &str = "RustSSH";
pub const APP_NAME: &str = "RustSSHClient";

#[derive(Debug, Clone, Default)]
pub struct StorageSnapshot {
    pub hosts: Vec<HostRecord>,
    pub keys: Vec<SshKeyRecord>,
}

#[derive(Debug, Clone)]
pub struct StorageFacade {
    root: PathBuf,
}

impl StorageFacade {
    pub fn new() -> Self {
        let root = ProjectDirs::from(APP_QUALIFIER, APP_ORGANIZATION, APP_NAME)
            .map(|dirs| dirs.data_local_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".rustssh"));

        Self { root }
    }

    pub fn for_root(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &PathBuf {
        &self.root
    }

    pub fn load_snapshot(&self) -> AppResult<StorageSnapshot> {
        let credentials_store = credentials::CredentialsStore::new(self.root.join("hosts.vault"));
        let mut hosts = credentials_store.load_hosts()?;
        let original_len = hosts.len();
        let now = Utc::now();
        hosts.retain(|host| !host.is_expired(now));

        if hosts.len() != original_len {
            credentials_store.save_hosts(&hosts)?;
        }

        Ok(StorageSnapshot {
            hosts,
            keys: keys::KeyStore::new(self.root.join("keys.vault")).load_keys()?,
        })
    }

    pub fn save_snapshot(&self, snapshot: &StorageSnapshot) -> AppResult<()> {
        credentials::CredentialsStore::new(self.root.join("hosts.vault"))
            .save_hosts(&snapshot.hosts)?;
        keys::KeyStore::new(self.root.join("keys.vault")).save_keys(&snapshot.keys)?;
        Ok(())
    }

    pub fn ensure_root(&self) -> AppResult<()> {
        std::fs::create_dir_all(&self.root).map_err(AppError::from)
    }
}
