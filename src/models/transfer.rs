use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferDirection {
    Upload,
    Download,
    Copy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferStatus {
    Queued,
    Running,
    Completed,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferProgress {
    pub id: Uuid,
    pub label: String,
    pub direction: TransferDirection,
    pub transferred_bytes: u64,
    pub total_bytes: u64,
    pub status: TransferStatus,
}

impl TransferProgress {
    pub fn queued(
        label: impl Into<String>,
        direction: TransferDirection,
        total_bytes: u64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            label: label.into(),
            direction,
            transferred_bytes: 0,
            total_bytes,
            status: TransferStatus::Queued,
        }
    }

    pub fn percent_complete(&self) -> f32 {
        if self.total_bytes == 0 {
            return 0.0;
        }

        (self.transferred_bytes as f32 / self.total_bytes as f32).clamp(0.0, 1.0)
    }
}
