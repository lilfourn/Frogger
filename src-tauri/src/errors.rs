use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
    pub recoverable: bool,
    pub details: Option<String>,
}

impl CommandError {
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        recoverable: bool,
        details: Option<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            recoverable,
            details,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new("internal_error", message, true, None)
    }

    pub fn unavailable(message: impl Into<String>, details: Option<String>) -> Self {
        Self::new("unavailable", message, true, details)
    }

    pub fn missing_path(message: impl Into<String>, details: Option<String>) -> Self {
        Self::new("missing_path", message, true, details)
    }

    pub fn permission_denied(message: impl Into<String>, details: Option<String>) -> Self {
        Self::new("permission_denied", message, true, details)
    }
}

impl From<anyhow::Error> for CommandError {
    fn from(error: anyhow::Error) -> Self {
        Self::internal(error.to_string())
    }
}
