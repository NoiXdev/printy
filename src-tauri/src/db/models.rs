use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WatchFolder {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub enabled: i64,
    pub poll_interval_secs: i64,
    pub file_types: String,
    pub printer_name: String,
    pub copies: i64,
    pub duplex: String,
    pub color_mode: String,
    pub post_action: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl WatchFolder {
    /// `file_types` is stored as a JSON array of lowercase extensions.
    pub fn types(&self) -> Vec<String> {
        serde_json::from_str(&self.file_types).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PrintJob {
    pub id: i64,
    pub folder_id: i64,
    pub file_path: String,
    pub file_name: String,
    pub size_bytes: i64,
    pub mtime_ms: i64,
    pub sha256: String,
    pub state: String,
    pub attempts: i64,
    pub printer_name: String,
    pub copies: i64,
    pub duplex: String,
    pub color_mode: String,
    pub error_kind: Option<String>,
    pub error_message: Option<String>,
    pub next_attempt_at: Option<i64>,
    pub enqueued_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}
