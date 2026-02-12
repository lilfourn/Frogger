use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("{0}")]
    General(String),

    #[error("OCR error: {0}")]
    Ocr(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("Watcher error: {0}")]
    Watcher(String),
}

impl AppError {
    pub fn capture(self) -> Self {
        sentry::capture_message(&self.to_string(), sentry::Level::Error);
        self
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
