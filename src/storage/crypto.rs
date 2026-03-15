use std::fs;
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use keyring::Error as KeyringError;
use rand::RngCore;
use rand::rngs::OsRng;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{AppError, AppResult};
use crate::storage::APP_NAME;

const MASTER_KEY_ACCOUNT: &str = "vault-master-key";
const MASTER_KEY_FALLBACK_FILE: &str = "master.key";

#[derive(Debug, Clone)]
pub enum KeySource {
    SystemKeyring,
    Static([u8; 32]),
}

#[derive(Debug, Clone)]
pub struct EncryptedJsonStore {
    path: PathBuf,
    key_source: KeySource,
}

impl EncryptedJsonStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            key_source: KeySource::SystemKeyring,
        }
    }

    pub fn with_key(path: PathBuf, key: [u8; 32]) -> Self {
        Self {
            path,
            key_source: KeySource::Static(key),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load_or_default<T>(&self) -> AppResult<T>
    where
        T: DeserializeOwned + Default,
    {
        if !self.path.exists() {
            return Ok(T::default());
        }

        let bytes = fs::read(&self.path)?;
        let plaintext = self.decrypt(&bytes)?;
        Ok(serde_json::from_slice::<T>(&plaintext)?)
    }

    pub fn save<T>(&self, value: &T) -> AppResult<()>
    where
        T: Serialize,
    {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let plaintext = serde_json::to_vec_pretty(value)?;
        let ciphertext = self.encrypt(&plaintext)?;
        fs::write(&self.path, ciphertext)?;
        Ok(())
    }

    fn encrypt(&self, plaintext: &[u8]) -> AppResult<Vec<u8>> {
        let key = self.resolve_key()?;
        let cipher =
            Aes256Gcm::new_from_slice(&key).map_err(|error| AppError::Crypto(error.to_string()))?;

        let mut nonce_bytes = [0_u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let mut ciphertext = cipher.encrypt(nonce, plaintext)?;
        let mut payload = nonce_bytes.to_vec();
        payload.append(&mut ciphertext);
        Ok(payload)
    }

    fn decrypt(&self, ciphertext: &[u8]) -> AppResult<Vec<u8>> {
        if ciphertext.len() < 12 {
            return Err(AppError::Crypto(
                "Encrypted vault payload is shorter than the nonce.".into(),
            ));
        }

        let key = self.resolve_key()?;
        let cipher =
            Aes256Gcm::new_from_slice(&key).map_err(|error| AppError::Crypto(error.to_string()))?;
        let (nonce_bytes, body) = ciphertext.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        cipher.decrypt(nonce, body).map_err(AppError::from)
    }

    fn resolve_key(&self) -> AppResult<[u8; 32]> {
        match self.key_source {
            KeySource::Static(key) => Ok(key),
            KeySource::SystemKeyring => load_or_create_master_key(&self.path),
        }
    }
}

fn load_or_create_master_key(vault_path: &Path) -> AppResult<[u8; 32]> {
    if let Some(key) = load_fallback_master_key(vault_path)? {
        return Ok(key);
    }

    let entry = keyring::Entry::new(APP_NAME, MASTER_KEY_ACCOUNT)?;

    match entry.get_password() {
        Ok(existing) => {
            let key = decode_master_key(&existing)?;
            sync_fallback_master_key(vault_path, &key)?;
            Ok(key)
        }
        Err(KeyringError::NoEntry) => create_master_key(&entry, vault_path),
        Err(KeyringError::NoStorageAccess(_)) | Err(KeyringError::PlatformFailure(_)) => {
            load_or_create_fallback_master_key(vault_path)
        }
        Err(error) => Err(error.into()),
    }
}

fn load_fallback_master_key(vault_path: &Path) -> AppResult<Option<[u8; 32]>> {
    let key_path = master_key_fallback_path(vault_path);
    if !key_path.exists() {
        return Ok(None);
    }

    let encoded = fs::read_to_string(&key_path)?;
    decode_master_key(encoded.trim()).map(Some)
}

fn create_master_key(entry: &keyring::Entry, vault_path: &Path) -> AppResult<[u8; 32]> {
    let key = generate_master_key();
    let encoded = BASE64.encode(key);

    match entry.set_password(&encoded) {
        Ok(()) => {
            sync_fallback_master_key(vault_path, &key)?;
            Ok(key)
        }
        Err(KeyringError::NoStorageAccess(_)) | Err(KeyringError::PlatformFailure(_)) => {
            persist_fallback_master_key(vault_path, &key)?;
            Ok(key)
        }
        Err(error) => Err(error.into()),
    }
}

fn load_or_create_fallback_master_key(vault_path: &Path) -> AppResult<[u8; 32]> {
    let key_path = master_key_fallback_path(vault_path);
    if key_path.exists() {
        let encoded = fs::read_to_string(&key_path)?;
        return decode_master_key(encoded.trim());
    }

    if vault_path.exists() {
        return Err(AppError::Crypto(
            "Existing encrypted storage cannot be opened because the master key is unavailable.".into(),
        ));
    }

    let key = generate_master_key();
    persist_fallback_master_key(vault_path, &key)?;
    Ok(key)
}

fn sync_fallback_master_key(vault_path: &Path, key: &[u8; 32]) -> AppResult<()> {
    persist_fallback_master_key(vault_path, key)
}

fn persist_fallback_master_key(vault_path: &Path, key: &[u8; 32]) -> AppResult<()> {
    let key_path = master_key_fallback_path(vault_path);
    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(key_path, BASE64.encode(key))?;
    Ok(())
}

fn master_key_fallback_path(vault_path: &Path) -> PathBuf {
    vault_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(MASTER_KEY_FALLBACK_FILE)
}

fn generate_master_key() -> [u8; 32] {
    let mut key = [0_u8; 32];
    OsRng.fill_bytes(&mut key);
    key
}

fn decode_master_key(encoded: &str) -> AppResult<[u8; 32]> {
    let raw = BASE64
        .decode(encoded)
        .map_err(|error| AppError::Crypto(error.to_string()))?;

    if raw.len() != 32 {
        return Err(AppError::Crypto(
            "Stored master key has an unexpected length.".into(),
        ));
    }

    let mut key = [0_u8; 32];
    key.copy_from_slice(&raw);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::EncryptedJsonStore;

    #[test]
    fn round_trips_encrypted_json() {
        let tempdir = tempdir().expect("create tempdir");
        let store = EncryptedJsonStore::with_key(tempdir.path().join("sample.vault"), [7; 32]);
        let payload = vec!["alpha".to_string(), "beta".to_string()];

        store.save(&payload).expect("save vault");
        let loaded: Vec<String> = store.load_or_default().expect("load vault");

        assert_eq!(loaded, payload);
    }

    #[test]
    fn fallback_master_key_is_stable_across_reads() {
        let tempdir = tempdir().expect("create tempdir");
        let vault_path = tempdir.path().join("hosts.vault");

        let first = super::load_or_create_fallback_master_key(&vault_path)
            .expect("create fallback master key");
        let second = super::load_or_create_fallback_master_key(&vault_path)
            .expect("reload fallback master key");

        assert_eq!(first, second);
    }

    #[test]
    fn syncs_fallback_master_key_from_existing_key() {
        let tempdir = tempdir().expect("create tempdir");
        let vault_path = tempdir.path().join("hosts.vault");
        let expected = [9_u8; 32];

        super::sync_fallback_master_key(&vault_path, &expected)
            .expect("sync fallback master key");
        let loaded = super::load_or_create_fallback_master_key(&vault_path)
            .expect("load synced fallback master key");

        assert_eq!(loaded, expected);
    }

    #[test]
    fn does_not_generate_new_fallback_key_for_existing_vault() {
        let tempdir = tempdir().expect("create tempdir");
        let vault_path = tempdir.path().join("hosts.vault");
        fs::write(&vault_path, b"existing encrypted payload").expect("write vault");

        let error = super::load_or_create_fallback_master_key(&vault_path)
            .expect_err("missing key should error");

        assert!(error
            .to_string()
            .contains("master key is unavailable"));
    }
}
