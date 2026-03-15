use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SshKeyRecord {
    pub id: Uuid,
    pub label: String,
    pub pem_contents: String,
    pub created_at: DateTime<Utc>,
}

impl fmt::Debug for SshKeyRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SshKeyRecord")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("pem_contents", &"<redacted>")
            .field("created_at", &self.created_at)
            .finish()
    }
}

impl SshKeyRecord {
    pub fn new(label: impl Into<String>, pem_contents: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            label: label.into(),
            pem_contents: pem_contents.into(),
            created_at: Utc::now(),
        }
    }
}
