use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileKind {
    Directory,
    File,
    Symlink,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub kind: FileKind,
    pub size: u64,
    pub permissions: String,
    pub owner: Option<String>,
    pub modified: Option<DateTime<Utc>>,
}

impl FileEntry {
    pub fn is_directory(&self) -> bool {
        matches!(self.kind, FileKind::Directory)
    }
}
