use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeInfo {
    pub name: String,
    pub path: String,
    pub total_bytes: Option<u64>,
    pub free_bytes: Option<u64>,
}
