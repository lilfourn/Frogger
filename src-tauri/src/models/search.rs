use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrRecord {
    pub file_path: String,
    pub text_content: String,
    pub language: String,
    pub confidence: Option<f64>,
    pub processed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub file_path: String,
    pub file_name: String,
    pub is_directory: bool,
    pub score: f64,
    pub match_source: String,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FtsResult {
    pub file_path: String,
}

#[derive(Debug, Clone)]
pub struct VecResult {
    pub file_path: String,
    #[allow(dead_code)]
    pub distance: f64,
}
