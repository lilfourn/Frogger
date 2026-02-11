use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationType {
    Move,
    Copy,
    Rename,
    Delete,
    CreateDir,
    BatchRename,
}

impl std::fmt::Display for OperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Move => write!(f, "move"),
            Self::Copy => write!(f, "copy"),
            Self::Rename => write!(f, "rename"),
            Self::Delete => write!(f, "delete"),
            Self::CreateDir => write!(f, "create_dir"),
            Self::BatchRename => write!(f, "batch_rename"),
        }
    }
}

impl std::str::FromStr for OperationType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "move" => Ok(Self::Move),
            "copy" => Ok(Self::Copy),
            "rename" => Ok(Self::Rename),
            "delete" => Ok(Self::Delete),
            "create_dir" => Ok(Self::CreateDir),
            "batch_rename" => Ok(Self::BatchRename),
            _ => Err(format!("unknown operation type: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationRecord {
    pub operation_id: String,
    pub operation_type: OperationType,
    pub forward_command: String,
    pub inverse_command: String,
    pub affected_paths: Vec<String>,
    pub metadata: Option<serde_json::Value>,
    pub executed_at: String,
    pub undone: bool,
}
