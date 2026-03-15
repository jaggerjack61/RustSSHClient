use std::fmt;

use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("SSH error: {0}")]
    Ssh(String),
    #[error("SFTP error: {0}")]
    Sftp(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Encryption error: {0}")]
    Crypto(String),
    #[error("Clipboard error: {0}")]
    Clipboard(String),
    #[error("Configuration error: {0}")]
    Configuration(String),
}

impl From<ssh2::Error> for AppError {
    fn from(value: ssh2::Error) -> Self {
        Self::Ssh(value.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::Storage(value.to_string())
    }
}

impl From<keyring::Error> for AppError {
    fn from(value: keyring::Error) -> Self {
        Self::Storage(value.to_string())
    }
}

impl From<arboard::Error> for AppError {
    fn from(value: arboard::Error) -> Self {
        Self::Clipboard(value.to_string())
    }
}

impl From<aes_gcm::Error> for AppError {
    fn from(value: aes_gcm::Error) -> Self {
        Self::Crypto(value.to_string())
    }
}

impl From<fmt::Error> for AppError {
    fn from(value: fmt::Error) -> Self {
        Self::Configuration(value.to_string())
    }
}
