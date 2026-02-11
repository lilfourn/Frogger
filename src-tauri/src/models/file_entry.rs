use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub extension: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub is_directory: bool,
    pub parent_path: Option<String>,
}
