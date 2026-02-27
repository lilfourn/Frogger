use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

pub const DEFAULT_MAX_FILES: usize = 20_000;
pub const DEFAULT_CHUNK_SIZE: usize = 80;
pub const HIGH_CONFIDENCE_THRESHOLD: f32 = 0.72;
pub const FAST_CLASSIFY_THRESHOLD: f32 = 0.80;
pub const MEDIUM_CONFIDENCE_THRESHOLD: f32 = 0.50;
pub const PACKING_DEFAULT_TARGET: usize = 40;
pub const PACKING_MIN_TARGET: usize = 20;
pub const PACKING_MAX_TARGET: usize = 60;
pub const PACKING_MAX_DEPTH: usize = 2;
const MAX_SNIPPET_CHARS: usize = 200;

/// Legacy canonical folder names — used only for backward-compatible skip logic
/// in `collect_indexed_manifest` to avoid re-organizing previously organized files.
const LEGACY_ORGANIZED_FOLDERS: &[&str] = &[
    "projects",
    "documents",
    "spreadsheets",
    "images",
    "videos",
    "audio",
    "archives",
    "data",
    "design",
    "system",
    "other",
];

#[derive(Debug, Clone, Serialize)]
pub struct IndexedManifestFile {
    pub absolute_path: String,
    pub relative_path: String,
    pub parent_relative: String,
    pub depth: usize,
    pub extension: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub modified_at: Option<String>,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizeCategory {
    pub folder: String,
    pub description: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizePlan {
    #[serde(default)]
    pub categories: Vec<OrganizeCategory>,
    #[serde(default)]
    pub placements: Vec<OrganizePlacement>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrganizePlanStats {
    pub total_files: usize,
    pub indexed_files: usize,
    pub skipped_hidden: usize,
    pub skipped_already_organized: usize,
    pub chunks: usize,
    pub other_count: usize,
    pub other_ratio: f64,
    pub deterministic_assigned: usize,
    pub fast_classified: usize,
    pub llm_assigned: usize,
    pub fallback_assigned: usize,
    pub parse_failed_chunks: usize,
    pub packed_directories: usize,
    pub max_children_observed: usize,
    pub avg_children_per_generated_dir: f64,
    pub capacity_overflow_dirs: usize,
    pub packing_llm_calls: usize,
    pub folders_over_target: usize,
    pub folders_over_hard_max: usize,
    pub avg_depth_generated: f64,
    pub fallback_label_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizePlacement {
    pub path: String,
    pub folder: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subfolder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packing_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_name: Option<String>,
    #[serde(default)]
    pub confidence: f32,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrganizePlanDocument {
    pub taxonomy_version: String,
    pub categories: Vec<OrganizeCategory>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub placements: Vec<OrganizePlacement>,
    pub unclassified: Vec<String>,
    pub stats: OrganizePlanStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrganizeAction {
    pub tool: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrganizeActionBatch {
    pub actions: Vec<OrganizeAction>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileTypeHint {
    Code,
    Document,
    Spreadsheet,
    Image,
    Video,
    Audio,
    Archive,
    Data,
    Design,
    System,
    Unknown,
}

impl FileTypeHint {
    pub fn default_folder_name(self) -> &'static str {
        match self {
            Self::Code => "projects",
            Self::Document => "documents",
            Self::Spreadsheet => "spreadsheets",
            Self::Image => "images",
            Self::Video => "videos",
            Self::Audio => "audio",
            Self::Archive => "archives",
            Self::Data => "data",
            Self::Design => "design",
            Self::System => "system",
            Self::Unknown => "uncategorized",
        }
    }

    pub fn hint_label(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Document => "document",
            Self::Spreadsheet => "spreadsheet",
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Archive => "archive",
            Self::Data => "data",
            Self::Design => "design",
            Self::System => "system",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeterministicClassification {
    pub type_hint: FileTypeHint,
    pub confidence: f32,
    pub strong_signal: bool,
}

#[derive(Debug, Clone)]
pub struct FastClassification {
    pub folder: String,
}

#[derive(Debug, Clone, Deserialize)]
struct OrganizeChunkAssignment {
    #[serde(default)]
    i: Option<usize>,
    #[serde(default)]
    file: Option<String>,
    folder: String,
    #[serde(default)]
    subfolder: Option<String>,
    #[serde(default)]
    suggested_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct OrganizeChunkPayload {
    #[serde(default)]
    categories: Vec<OrganizeCategory>,
    #[serde(default)]
    assignments: Vec<OrganizeChunkAssignment>,
}

#[derive(Debug, Clone)]
pub struct ParsedChunkPlan {
    pub category_map: HashMap<String, Vec<String>>,
    pub subfolders: HashMap<String, Option<String>>,
    pub suggested_names: HashMap<String, Option<String>>,
    pub fallback_paths: HashSet<String>,
}

#[derive(Debug, Clone, Default)]
pub struct OrganizePackingStats {
    pub packed_directories: usize,
    pub max_children_observed: usize,
    pub avg_children_per_generated_dir: f64,
    pub capacity_overflow_dirs: usize,
    pub packing_llm_calls: usize,
    pub folders_over_target: usize,
    pub folders_over_hard_max: usize,
    pub avg_depth_generated: f64,
    pub fallback_label_rate: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct PackingPolicy {
    pub min_children_target: usize,
    pub target_children: usize,
    pub max_children_target: usize,
    pub max_depth: usize,
}

impl Default for PackingPolicy {
    fn default() -> Self {
        Self {
            min_children_target: PACKING_MIN_TARGET,
            target_children: PACKING_DEFAULT_TARGET,
            max_children_target: PACKING_MAX_TARGET,
            max_depth: PACKING_MAX_DEPTH,
        }
    }
}

fn normalize_path(path: &str) -> String {
    let mut out = path.replace('\\', "/");
    while out.ends_with('/') && out.len() > 1 {
        out.pop();
    }
    out
}

fn relative_from_root(root: &str, absolute: &str) -> Option<String> {
    if root == "/" {
        return absolute.strip_prefix('/').map(|s| s.to_string());
    }
    absolute
        .strip_prefix(&(root.to_string() + "/"))
        .map(|s| s.to_string())
}

fn is_hidden_relative(relative: &str) -> bool {
    relative.split('/').any(|segment| segment.starts_with('.'))
}

fn first_segment(relative: &str) -> Option<&str> {
    relative.split('/').find(|segment| !segment.is_empty())
}

fn compact_snippet(raw: &str) -> Option<String> {
    let compact = raw
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if compact.is_empty() {
        return None;
    }
    Some(compact.chars().take(MAX_SNIPPET_CHARS).collect())
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn normalize_label(label: &str) -> String {
    label
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c
            } else if c == ' ' || c == '-' || c == '_' || c == '/' || c == '\\' {
                '_'
            } else {
                '\0'
            }
        })
        .filter(|c| *c != '\0')
        .collect::<String>()
        .split('_')
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn normalize_relative_path(path: &str) -> String {
    let normalized = normalize_path(path.trim());
    normalized
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string()
}

pub fn validate_folder_name(folder: &str) -> String {
    let normalized = normalize_label(folder);
    if normalized.is_empty() {
        return "uncategorized".to_string();
    }
    if normalized.len() > 50 {
        return normalized.chars().take(50).collect();
    }
    normalized
}

fn validate_subfolder(candidate: &str) -> Option<String> {
    let normalized = normalize_label(candidate);
    if normalized.is_empty() {
        return None;
    }
    Some(if normalized.len() > 50 {
        normalized.chars().take(50).collect()
    } else {
        normalized
    })
}

pub fn validate_suggested_name(name: &str, original_path: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
        || trimmed.starts_with('.')
    {
        return None;
    }
    let cleaned: String = trimmed
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                c
            } else if c == ' ' {
                '_'
            } else {
                '\0'
            }
        })
        .filter(|c| *c != '\0')
        .collect();
    if cleaned.is_empty() || cleaned.starts_with('.') {
        return None;
    }
    let original_ext = Path::new(original_path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase());
    let suggested_stem = Path::new(&cleaned)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    if suggested_stem.is_empty() {
        return None;
    }
    let result = match original_ext {
        Some(ext) => format!("{suggested_stem}.{ext}"),
        None => suggested_stem,
    };
    let original_name = Path::new(original_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if result == original_name {
        return None;
    }
    Some(result)
}

fn normalize_component(component: &str) -> Option<String> {
    let value = normalize_label(component);
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn normalize_packing_path(path: &str) -> Option<String> {
    let normalized = normalize_path(path.trim())
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_string();
    if normalized.is_empty() {
        return None;
    }
    let segments = normalized
        .split('/')
        .filter_map(normalize_component)
        .collect::<Vec<_>>();
    if segments.is_empty() {
        None
    } else {
        Some(segments.join("/"))
    }
}

fn type_hint_from_extension(extension: &str) -> Option<FileTypeHint> {
    match extension {
        "rs" | "js" | "jsx" | "ts" | "tsx" | "py" | "go" | "java" | "kt" | "swift" | "c" | "cc"
        | "cpp" | "h" | "hpp" | "sh" | "zsh" | "bash" | "ps1" | "toml" | "yaml" | "yml" | "ini"
        | "env" | "lock" => Some(FileTypeHint::Code),
        "txt" | "md" | "doc" | "docx" | "pdf" | "rtf" | "odt" | "pages" | "eml" | "msg" | "ppt"
        | "pptx" | "key" => Some(FileTypeHint::Document),
        "csv" | "tsv" | "xls" | "xlsx" | "ods" | "numbers" => Some(FileTypeHint::Spreadsheet),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "heic" | "tif" | "tiff" => {
            Some(FileTypeHint::Image)
        }
        "mp4" | "mov" | "avi" | "mkv" | "webm" | "m4v" => Some(FileTypeHint::Video),
        "mp3" | "wav" | "aac" | "flac" | "ogg" | "m4a" => Some(FileTypeHint::Audio),
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" => Some(FileTypeHint::Archive),
        "json" | "jsonl" | "parquet" | "feather" | "db" | "sqlite" | "log" | "ndjson" => {
            Some(FileTypeHint::Data)
        }
        "fig" | "xd" | "sketch" | "ai" | "psd" | "eps" => Some(FileTypeHint::Design),
        "ds_store" => Some(FileTypeHint::System),
        _ => None,
    }
}

fn type_hint_from_mime(mime: &str) -> Option<FileTypeHint> {
    if mime.starts_with("image/") {
        return Some(FileTypeHint::Image);
    }
    if mime.starts_with("video/") {
        return Some(FileTypeHint::Video);
    }
    if mime.starts_with("audio/") {
        return Some(FileTypeHint::Audio);
    }
    if mime == "message/rfc822" {
        return Some(FileTypeHint::Document);
    }
    if mime.contains("spreadsheet") || mime.contains("excel") || mime.contains("csv") {
        return Some(FileTypeHint::Spreadsheet);
    }
    if mime.contains("zip") || mime.contains("compressed") || mime.contains("tar") {
        return Some(FileTypeHint::Archive);
    }
    if mime.contains("json") || mime.contains("sqlite") || mime.contains("parquet") {
        return Some(FileTypeHint::Data);
    }
    if mime.contains("pdf")
        || mime.contains("msword")
        || mime.contains("officedocument.wordprocessingml")
    {
        return Some(FileTypeHint::Document);
    }
    None
}

fn add_hint_score(scores: &mut HashMap<FileTypeHint, f32>, hint: FileTypeHint, value: f32) {
    let entry = scores.entry(hint).or_insert(0.0);
    *entry = (*entry + value).min(1.0);
}

pub fn classify_file_deterministic(file: &IndexedManifestFile) -> DeterministicClassification {
    let mut scores: HashMap<FileTypeHint, f32> = HashMap::new();
    let relative_text = file.relative_path.to_lowercase();
    let parent_text = file.parent_relative.to_lowercase();
    let snippet_text = file.snippet.as_deref().unwrap_or_default().to_lowercase();
    let extension = file
        .extension
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let mime = file
        .mime_type
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let path_text = format!("{parent_text} {relative_text}");

    // Path keyword signals
    if contains_any(
        &path_text,
        &[
            "project", "code", "src", "source", "dev", "repo", "app", "software",
        ],
    ) {
        add_hint_score(&mut scores, FileTypeHint::Code, 0.45);
    }
    if contains_any(
        &path_text,
        &[
            "doc", "docs", "text", "note", "notes", "writing", "draft", "letter",
        ],
    ) {
        add_hint_score(&mut scores, FileTypeHint::Document, 0.45);
    }
    if contains_any(
        &path_text,
        &["image", "photo", "picture", "img", "screenshot"],
    ) {
        add_hint_score(&mut scores, FileTypeHint::Image, 0.45);
    }
    if contains_any(&path_text, &["video", "movie", "clips", "footage"]) {
        add_hint_score(&mut scores, FileTypeHint::Video, 0.45);
    }
    if contains_any(&path_text, &["audio", "sound", "music", "songs", "podcast"]) {
        add_hint_score(&mut scores, FileTypeHint::Audio, 0.45);
    }
    if contains_any(&path_text, &["archive", "backup", "zip", "compressed"]) {
        add_hint_score(&mut scores, FileTypeHint::Archive, 0.45);
    }
    if contains_any(
        &path_text,
        &["spreadsheet", "finance", "budget", "accounting", "excel"],
    ) {
        add_hint_score(&mut scores, FileTypeHint::Spreadsheet, 0.45);
    }
    if contains_any(
        &path_text,
        &["data", "dataset", "database", "export", "log", "logs"],
    ) {
        add_hint_score(&mut scores, FileTypeHint::Data, 0.45);
    }
    if contains_any(
        &path_text,
        &["design", "asset", "wireframe", "mock", "prototype", "brand"],
    ) {
        add_hint_score(&mut scores, FileTypeHint::Design, 0.45);
    }
    if contains_any(
        &path_text,
        &["system", "hidden", "config", "configuration", "settings"],
    ) {
        add_hint_score(&mut scores, FileTypeHint::System, 0.45);
    }

    // Semantic content signals from path
    if contains_any(
        &relative_text,
        &[
            "invoice",
            "receipt",
            "statement",
            "contract",
            "agreement",
            "legal",
            "email",
            "mail",
        ],
    ) {
        add_hint_score(&mut scores, FileTypeHint::Document, 0.25);
    }
    if contains_any(&relative_text, &["dataset", "export", "dump", "trace"]) {
        add_hint_score(&mut scores, FileTypeHint::Data, 0.25);
    }
    if contains_any(&relative_text, &["backup", "snapshot"]) {
        add_hint_score(&mut scores, FileTypeHint::Archive, 0.25);
    }

    // Extension/MIME signals
    let mut strong_signal = false;
    if let Some(hint) = type_hint_from_extension(&extension) {
        add_hint_score(&mut scores, hint, 0.25);
        strong_signal = true;
    }
    if let Some(hint) = type_hint_from_mime(&mime) {
        add_hint_score(&mut scores, hint, 0.25);
        strong_signal = true;
    }

    // Snippet content signals
    if contains_any(
        &snippet_text,
        &[
            "invoice",
            "receipt",
            "subtotal",
            "tax",
            "amount",
            "agreement",
            "confidential",
        ],
    ) {
        add_hint_score(&mut scores, FileTypeHint::Document, 0.05);
    }
    if contains_any(
        &snippet_text,
        &["error", "traceback", "stack", "query", "dataset"],
    ) {
        add_hint_score(&mut scores, FileTypeHint::Data, 0.05);
    }

    let (type_hint, confidence) = scores
        .into_iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(hint, score)| (hint, score.min(1.0)))
        .unwrap_or((FileTypeHint::Unknown, 0.0));

    DeterministicClassification {
        type_hint,
        confidence,
        strong_signal,
    }
}

pub fn classify_manifest_deterministic(
    files: &[IndexedManifestFile],
) -> HashMap<String, DeterministicClassification> {
    let mut out = HashMap::new();
    for file in files {
        out.insert(
            file.relative_path.clone(),
            classify_file_deterministic(file),
        );
    }
    out
}

const GENERIC_PARENT_NAMES: &[&str] = &[
    "downloads",
    "desktop",
    "tmp",
    "temp",
    "files",
    "stuff",
    "new folder",
    "new_folder",
    "untitled folder",
    "untitled_folder",
    "misc",
    "random",
    "unsorted",
    "",
];

fn is_generic_parent(name: &str) -> bool {
    GENERIC_PARENT_NAMES.contains(&name.to_lowercase().trim())
}

fn smart_folder_name(file: &IndexedManifestFile, det: &DeterministicClassification) -> String {
    let parent = file.parent_relative.trim().trim_matches('/');
    // Use the immediate parent directory name (last segment)
    let last_segment = parent.rsplit('/').next().unwrap_or(parent);
    if !last_segment.is_empty() && !is_generic_parent(last_segment) {
        return normalize_label(last_segment);
    }
    det.type_hint.default_folder_name().to_string()
}

pub fn classify_file_fast(file: &IndexedManifestFile) -> Option<FastClassification> {
    let det = classify_file_deterministic(file);
    if det.confidence < FAST_CLASSIFY_THRESHOLD || !det.strong_signal {
        return None;
    }
    match det.type_hint {
        FileTypeHint::Image
        | FileTypeHint::Video
        | FileTypeHint::Audio
        | FileTypeHint::Archive
        | FileTypeHint::Code => {
            let folder = smart_folder_name(file, &det);
            Some(FastClassification { folder })
        }
        // Documents, Spreadsheets, Data, Design -- send to LLM for descriptive naming
        _ => None,
    }
}

fn fallback_category_for_path(
    rel_path: &str,
    deterministic: &HashMap<String, DeterministicClassification>,
) -> (String, Option<String>) {
    let Some(suggestion) = deterministic.get(rel_path) else {
        return ("uncategorized".to_string(), None);
    };

    if suggestion.type_hint != FileTypeHint::Unknown
        && (suggestion.confidence >= MEDIUM_CONFIDENCE_THRESHOLD || suggestion.strong_signal)
    {
        return (suggestion.type_hint.default_folder_name().to_string(), None);
    }
    ("uncategorized".to_string(), None)
}

fn extension_family(extension: &str) -> &'static str {
    match extension {
        "pdf" => "pdf",
        "eml" | "msg" | "mbox" => "mail",
        "csv" | "tsv" | "xls" | "xlsx" | "ods" | "numbers" => "sheet",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "heic" | "tif" | "tiff" => {
            "image"
        }
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" => "archive",
        "json" | "jsonl" | "parquet" | "feather" | "db" | "sqlite" | "log" | "ndjson" | "xml"
        | "yml" | "yaml" => "data",
        "ppt" | "pptx" | "key" => "slides",
        "doc" | "docx" | "txt" | "rtf" | "md" | "odt" | "pages" | "tex" => "text",
        "mp4" | "mov" | "avi" | "mkv" | "webm" | "m4v" => "video",
        "mp3" | "wav" | "aac" | "flac" | "ogg" | "m4a" => "audio",
        "fig" | "xd" | "sketch" | "ai" | "psd" | "eps" => "design",
        _ => "misc",
    }
}

fn parse_year_month(year_text: &str, month_text: &str) -> Option<String> {
    if year_text.len() != 4 || !year_text.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if month_text.len() != 2 || !month_text.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let month = month_text.parse::<u8>().ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }
    Some(format!("{year_text}-{month_text}"))
}

fn extract_year_month_from_text(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    if bytes.len() < 6 {
        return None;
    }

    for idx in 0..bytes.len().saturating_sub(5) {
        if idx + 7 <= bytes.len() {
            let candidate = &value[idx..idx + 7];
            let parts = candidate.as_bytes();
            if parts[0..4].iter().all(|b| b.is_ascii_digit())
                && (parts[4] == b'-' || parts[4] == b'_' || parts[4] == b'/')
                && parts[5..7].iter().all(|b| b.is_ascii_digit())
            {
                if let Some(parsed) = parse_year_month(&candidate[0..4], &candidate[5..7]) {
                    return Some(parsed);
                }
            }
        }
        if idx + 6 <= bytes.len() {
            let candidate = &value[idx..idx + 6];
            if candidate.as_bytes().iter().all(|b| b.is_ascii_digit()) {
                if let Some(parsed) = parse_year_month(&candidate[0..4], &candidate[4..6]) {
                    return Some(parsed);
                }
            }
        }
    }
    None
}

fn modified_month(file: &IndexedManifestFile) -> String {
    if let Some(value) = file.modified_at.as_deref() {
        if let Some(month) = extract_year_month_from_text(value) {
            return month;
        }
    }
    if let Some(month) = extract_year_month_from_text(&file.relative_path) {
        return month;
    }
    "undated".to_string()
}

fn normalized_token(token: &str) -> Option<String> {
    if token.len() < 3 {
        return None;
    }
    if token.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    normalize_component(token)
}

fn semantic_token(file: &IndexedManifestFile) -> (String, bool) {
    const STOPWORDS: &[&str] = &[
        "the",
        "and",
        "for",
        "with",
        "from",
        "file",
        "document",
        "copy",
        "final",
        "draft",
        "signed",
        "scan",
        "image",
        "img",
        "new",
        "old",
        "invoice",
        "receipt",
        "statement",
        "order",
    ];
    const GENERIC: &[&str] = &[
        "bucket",
        "file",
        "files",
        "misc",
        "unknown",
        "document",
        "documents",
        "data",
        "item",
        "items",
        "report",
        "reports",
    ];

    let path = normalize_relative_path(&file.relative_path);
    let file_name = Path::new(&path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut tokens = file_name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .map(|token| token.trim().to_lowercase())
        .filter(|token| !STOPWORDS.contains(&token.as_str()))
        .filter_map(|token| normalized_token(&token))
        .collect::<Vec<_>>();

    let parent_tokens = Path::new(&path)
        .parent()
        .map(|parent| {
            parent
                .to_string_lossy()
                .split('/')
                .rev()
                .take(2)
                .flat_map(|segment| {
                    segment
                        .split(|c: char| !c.is_ascii_alphanumeric())
                        .map(|token| token.trim().to_lowercase())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    tokens.extend(
        parent_tokens
            .into_iter()
            .filter(|token| !STOPWORDS.contains(&token.as_str()))
            .filter_map(|token| normalized_token(&token)),
    );

    if let Some(snippet) = file.snippet.as_deref() {
        tokens.extend(
            snippet
                .split(|c: char| !c.is_ascii_alphanumeric())
                .take(18)
                .map(|token| token.trim().to_lowercase())
                .filter(|token| !STOPWORDS.contains(&token.as_str()))
                .filter_map(|token| normalized_token(&token)),
        );
    }

    tokens.retain(|token| !GENERIC.contains(&token.as_str()));

    if tokens.is_empty() {
        let parent = Path::new(&path)
            .parent()
            .and_then(|parent| parent.file_name())
            .map(|segment| segment.to_string_lossy().to_string())
            .unwrap_or_else(|| "bucket".to_string());
        if let Some(parent_token) = normalize_component(&parent.to_lowercase()) {
            if !GENERIC.contains(&parent_token.as_str()) {
                return (parent_token, false);
            }
        }
        return ("bucket".to_string(), true);
    }

    (tokens[0].clone(), false)
}

#[derive(Debug, Clone)]
struct PackingFeatures {
    semantic: String,
    month: String,
    family: String,
    fallback_label: bool,
}

fn packing_features(file: &IndexedManifestFile) -> PackingFeatures {
    let (semantic, semantic_fallback) = semantic_token(file);
    let month = modified_month(file);
    let extension = file
        .extension
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let family = extension_family(&extension).to_string();
    let fallback_label = semantic_fallback || family == "misc" || month == "undated";
    PackingFeatures {
        semantic,
        month,
        family,
        fallback_label,
    }
}

fn packed_bucket_label(features: &PackingFeatures) -> String {
    let semantic = normalize_component(&features.semantic).unwrap_or_else(|| "bucket".to_string());
    let month_component =
        normalize_component(&features.month).unwrap_or_else(|| "undated".to_string());
    let family_component =
        normalize_component(&features.family).unwrap_or_else(|| "misc".to_string());
    let base = format!("{semantic}_{month_component}_{family_component}");
    if base.len() <= 48 {
        base
    } else {
        let prefix = base.chars().take(38).collect::<String>();
        format!("{prefix}_{}", base.chars().count())
    }
}

fn bucket_ambiguity_score(features: &[PackingFeatures]) -> f64 {
    if features.is_empty() {
        return 0.0;
    }
    let total = features.len() as f64;
    let fallback_ratio = features.iter().filter(|item| item.fallback_label).count() as f64 / total;
    let undated_ratio = features
        .iter()
        .filter(|item| item.month == "undated")
        .count() as f64
        / total;
    let misc_ratio = features.iter().filter(|item| item.family == "misc").count() as f64 / total;
    (fallback_ratio * 0.55 + undated_ratio * 0.20 + misc_ratio * 0.25).clamp(0.0, 1.0)
}

fn base_target_for_folder(folder: &str, policy: PackingPolicy) -> usize {
    match folder {
        "documents" | "spreadsheets" | "data" => 28,
        "projects" | "design" => 24,
        "images" | "videos" | "audio" => 36,
        "archives" | "system" => 34,
        "other" => 22,
        _ => policy.target_children,
    }
}

fn adaptive_target_for_bucket(
    folder: &str,
    total_files: usize,
    ambiguity: f64,
    policy: PackingPolicy,
) -> usize {
    let mut target = base_target_for_folder(folder, policy) as i32;
    if ambiguity >= 0.80 {
        target -= 4;
    } else if ambiguity >= 0.60 {
        target -= 2;
    }
    if total_files >= 400 {
        target += 4;
    } else if total_files >= 150 {
        target += 2;
    }
    target.clamp(
        policy.min_children_target as i32,
        policy.max_children_target as i32,
    ) as usize
}

/// Flat packing: groups files by semantic token, then splits large groups by date.
/// Max 2 levels deep, no s01/s02 suffixes.
fn pack_bucket_flat(
    indices: &[usize],
    target: usize,
    policy: PackingPolicy,
    features: &HashMap<usize, PackingFeatures>,
    assignments: &mut HashMap<usize, String>,
) {
    if indices.is_empty() {
        return;
    }

    // Group by semantic token
    let mut by_semantic: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for &idx in indices {
        let key = features
            .get(&idx)
            .map(|f| normalize_component(&f.semantic).unwrap_or_else(|| "bucket".to_string()))
            .unwrap_or_else(|| "bucket".to_string());
        by_semantic.entry(key).or_default().push(idx);
    }

    for (semantic_key, group) in by_semantic {
        if group.len() <= target {
            for &idx in &group {
                assignments.insert(idx, semantic_key.clone());
            }
            continue;
        }

        // Split large semantic groups by year-month
        let mut by_month: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for &idx in &group {
            let month = features
                .get(&idx)
                .map(|f| normalize_component(&f.month).unwrap_or_else(|| "undated".to_string()))
                .unwrap_or_else(|| "undated".to_string());
            by_month.entry(month).or_default().push(idx);
        }

        if by_month.len() <= 1 {
            // Can't split further meaningfully, just chunk by max
            for chunk in group.chunks(policy.max_children_target.max(1)) {
                let label = semantic_key.clone();
                for &idx in chunk {
                    assignments.insert(idx, label.clone());
                }
            }
            continue;
        }

        for (month_key, month_group) in by_month {
            let label = format!("{semantic_key}/{month_key}");
            for &idx in &month_group {
                assignments.insert(idx, label.clone());
            }
        }
    }
}

fn clamp_packing_depth(path: &str, max_depth: usize) -> Option<String> {
    if max_depth == 0 {
        return None;
    }
    let normalized = normalize_packing_path(path)?;
    let mut segments = normalized
        .split('/')
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    if segments.len() <= max_depth {
        return Some(segments.join("/"));
    }

    let mut kept = segments.drain(..max_depth).collect::<Vec<_>>();
    let overflow = segments.join("_");
    if let Some(last) = kept.last_mut() {
        *last = normalize_component(&format!("{last}_{overflow}")).unwrap_or_else(|| last.clone());
    }
    Some(kept.join("/"))
}

pub fn apply_capacity_packing_with_policy(
    placements: Vec<OrganizePlacement>,
    manifest: &[IndexedManifestFile],
    policy: PackingPolicy,
) -> (Vec<OrganizePlacement>, OrganizePackingStats) {
    let mut out = placements
        .into_iter()
        .map(|mut placement| {
            placement.path = normalize_relative_path(&placement.path);
            placement.folder = validate_folder_name(&placement.folder);
            placement.subfolder = placement.subfolder.as_deref().and_then(validate_subfolder);
            placement.packing_path = placement
                .packing_path
                .as_deref()
                .and_then(normalize_packing_path);
            placement
        })
        .collect::<Vec<_>>();

    let manifest_index = manifest
        .iter()
        .map(|file| (normalize_relative_path(&file.relative_path), file))
        .collect::<HashMap<_, _>>();

    let mut buckets: BTreeMap<(String, Option<String>), Vec<usize>> = BTreeMap::new();
    for (idx, placement) in out.iter().enumerate() {
        if placement.folder == "system" {
            continue;
        }
        buckets
            .entry((placement.folder.clone(), placement.subfolder.clone()))
            .or_default()
            .push(idx);
    }

    let mut stats = OrganizePackingStats::default();
    let mut packed_sum_children = 0usize;
    let mut packed_sum_depth = 0usize;
    let mut total_featured_files = 0usize;
    let mut total_fallback_labels = 0usize;

    for ((folder, _subfolder), mut indices) in buckets {
        indices.sort_by(|a, b| out[*a].path.cmp(&out[*b].path));
        let total_children = indices.len();
        if total_children <= 1 {
            stats.max_children_observed = stats.max_children_observed.max(total_children);
            continue;
        }

        let mut features_by_idx: HashMap<usize, PackingFeatures> = HashMap::new();
        let mut feature_rows = Vec::new();
        for idx in &indices {
            let features = manifest_index
                .get(&out[*idx].path)
                .map(|file| packing_features(file))
                .unwrap_or_else(|| PackingFeatures {
                    semantic: "bucket".to_string(),
                    month: "undated".to_string(),
                    family: "misc".to_string(),
                    fallback_label: true,
                });
            if features.fallback_label {
                total_fallback_labels += 1;
            }
            total_featured_files += 1;
            feature_rows.push(features.clone());
            features_by_idx.insert(*idx, features);
        }

        let ambiguity = bucket_ambiguity_score(&feature_rows);
        let target = adaptive_target_for_bucket(&folder, total_children, ambiguity, policy);
        if total_children <= target {
            stats.max_children_observed = stats.max_children_observed.max(total_children);
            continue;
        }

        let mut assignments: HashMap<usize, String> = HashMap::new();
        pack_bucket_flat(&indices, target, policy, &features_by_idx, &mut assignments);

        let mut generated_indices: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for idx in indices {
            let label = assignments
                .get(&idx)
                .and_then(|value| clamp_packing_depth(value, policy.max_depth))
                .or_else(|| {
                    out[idx]
                        .packing_path
                        .as_deref()
                        .and_then(|value| clamp_packing_depth(value, policy.max_depth))
                })
                .or_else(|| {
                    features_by_idx
                        .get(&idx)
                        .map(packed_bucket_label)
                        .and_then(|value| clamp_packing_depth(&value, policy.max_depth))
                })
                .unwrap_or_else(|| "bucket_undated_misc".to_string());
            out[idx].packing_path = Some(label.clone());
            generated_indices.entry(label).or_default().push(idx);
        }

        loop {
            let over_hard_max = generated_indices
                .iter()
                .find(|(_, group)| group.len() > policy.max_children_target)
                .map(|(label, _)| label.clone());
            let Some(label) = over_hard_max else {
                break;
            };
            let Some(mut group_indices) = generated_indices.remove(&label) else {
                continue;
            };
            group_indices.sort_by(|a, b| out[*a].path.cmp(&out[*b].path));
            for (chunk_idx, chunk) in group_indices
                .chunks(policy.max_children_target.max(1))
                .enumerate()
            {
                let chunk_label = format!("{label}_part{}", chunk_idx + 1);
                for idx in chunk {
                    out[*idx].packing_path = Some(chunk_label.clone());
                }
                generated_indices
                    .entry(chunk_label)
                    .or_default()
                    .extend(chunk.iter().copied());
            }
        }

        let generated_counts = generated_indices
            .iter()
            .map(|(label, members)| (label.clone(), members.len()))
            .collect::<BTreeMap<_, _>>();
        stats.packed_directories += generated_counts.len();
        packed_sum_children += generated_counts.values().sum::<usize>();
        packed_sum_depth += generated_counts
            .keys()
            .map(|key| key.split('/').count())
            .sum::<usize>();
        stats.capacity_overflow_dirs += generated_counts
            .values()
            .filter(|count| **count > target)
            .count();
        stats.folders_over_target += generated_counts
            .values()
            .filter(|count| **count > target)
            .count();
        stats.folders_over_hard_max += generated_counts
            .values()
            .filter(|count| **count > policy.max_children_target)
            .count();
        stats.max_children_observed = stats
            .max_children_observed
            .max(generated_counts.values().copied().max().unwrap_or(0));
    }

    if stats.packed_directories > 0 {
        stats.avg_children_per_generated_dir =
            packed_sum_children as f64 / stats.packed_directories as f64;
        stats.avg_depth_generated = packed_sum_depth as f64 / stats.packed_directories as f64;
    }
    if total_featured_files > 0 {
        stats.fallback_label_rate = total_fallback_labels as f64 / total_featured_files as f64;
    }

    (out, stats)
}

pub fn apply_capacity_packing(
    placements: Vec<OrganizePlacement>,
    manifest: &[IndexedManifestFile],
) -> (Vec<OrganizePlacement>, OrganizePackingStats) {
    apply_capacity_packing_with_policy(placements, manifest, PackingPolicy::default())
}

pub fn collect_indexed_manifest(
    conn: &Connection,
    root_path: &str,
    include_hidden: bool,
    max_files: usize,
) -> Result<(Vec<IndexedManifestFile>, usize, usize), AppError> {
    let root = normalize_path(root_path);
    let slash_pattern = format!("{root}/%");
    let root_backslash = root.replace('/', "\\");
    let backslash_pattern = format!("{root_backslash}\\%");

    let mut stmt = conn.prepare(
        "SELECT
            f.path,
            f.extension,
            f.mime_type,
            f.size_bytes,
            f.modified_at,
            o.text_content
         FROM files f
         LEFT JOIN ocr_text o ON o.file_path = f.path
         WHERE f.is_directory = 0
           AND (
             f.path = ?1
             OR f.path LIKE ?2
             OR f.path = ?3
             OR f.path LIKE ?4
           )
         ORDER BY length(f.path) ASC, f.path ASC",
    )?;

    let mut all = Vec::new();
    let mut skipped_hidden = 0usize;
    let mut skipped_already_organized = 0usize;

    let rows = stmt.query_map(
        params![root, slash_pattern, root_backslash, backslash_pattern],
        |row| {
            let absolute_path: String = row.get(0)?;
            let extension: Option<String> = row.get(1)?;
            let mime_type: Option<String> = row.get(2)?;
            let size_bytes: Option<i64> = row.get(3)?;
            let modified_at: Option<String> = row.get(4)?;
            let raw_snippet: Option<String> = row.get(5)?;
            Ok((
                absolute_path,
                extension,
                mime_type,
                size_bytes,
                modified_at,
                raw_snippet,
            ))
        },
    )?;

    for row in rows {
        let (absolute_path, extension, mime_type, size_bytes, modified_at, raw_snippet) = row?;
        let absolute_normalized = normalize_path(&absolute_path);
        let Some(relative_path) = relative_from_root(&root, &absolute_normalized) else {
            continue;
        };
        if relative_path.is_empty() {
            continue;
        }

        if !include_hidden && is_hidden_relative(&relative_path) {
            skipped_hidden += 1;
            continue;
        }

        if let Some(segment) = first_segment(&relative_path) {
            if LEGACY_ORGANIZED_FOLDERS.contains(&segment) {
                skipped_already_organized += 1;
                continue;
            }
        }

        let parent_relative = Path::new(&relative_path)
            .parent()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let depth = relative_path.split('/').count().saturating_sub(1);
        let snippet = raw_snippet.as_deref().and_then(compact_snippet);

        all.push(IndexedManifestFile {
            absolute_path,
            relative_path,
            parent_relative,
            depth,
            extension,
            mime_type,
            size_bytes,
            modified_at,
            snippet,
        });
    }

    all.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    all.truncate(max_files);

    Ok((all, skipped_hidden, skipped_already_organized))
}

pub fn chunk_manifest(
    files: &[IndexedManifestFile],
    chunk_size: usize,
) -> Vec<Vec<IndexedManifestFile>> {
    if files.is_empty() {
        return Vec::new();
    }
    let size = chunk_size.max(1);
    files.chunks(size).map(|chunk| chunk.to_vec()).collect()
}

pub fn build_tree_summary(files: &[IndexedManifestFile]) -> serde_json::Value {
    let mut top_level_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut extension_counts: BTreeMap<String, usize> = BTreeMap::new();

    for file in files {
        let top = first_segment(&file.relative_path)
            .unwrap_or("(root)")
            .to_string();
        *top_level_counts.entry(top).or_default() += 1;

        let ext = file
            .extension
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("(none)")
            .to_lowercase();
        *extension_counts.entry(ext).or_default() += 1;
    }

    serde_json::json!({
        "top_level_counts": top_level_counts,
        "extension_counts": extension_counts,
    })
}

pub fn build_chunk_prompt(
    root_path: &str,
    chunk_index: usize,
    chunk_total: usize,
    tree_summary: &serde_json::Value,
    chunk_files: &[IndexedManifestFile],
    suggestions: &HashMap<String, DeterministicClassification>,
) -> Result<String, AppError> {
    let manifest_payload = chunk_files
        .iter()
        .enumerate()
        .map(|file| {
            let (i, file) = file;
            let suggestion = suggestions.get(&file.relative_path);
            serde_json::json!({
                "i": i,
                "relative_path": file.relative_path,
                "parent_relative": file.parent_relative,
                "extension": file.extension,
                "mime_type": file.mime_type,
                "snippet": file.snippet,
                "type_hint": suggestion.map(|s| s.type_hint.hint_label()),
            })
        })
        .collect::<Vec<_>>();
    let json_files = serde_json::to_string_pretty(&manifest_payload)?;
    let summary_json = serde_json::to_string_pretty(tree_summary)?;

    Ok(format!(
        "You are organizing a file tree using indexed file metadata and snippets.\n\
Root path: {root_path}\n\
Chunk: {chunk_index}/{chunk_total}\n\
\n\
Suggest descriptive, content-based folder names using snake_case.\n\
Group related files together. Choose names that describe the content,\n\
not just the file type (e.g., \"tax_returns\" not \"documents\",\n\
\"vacation_photos_2024\" not \"images\", \"website_source\" not \"projects\").\n\
Keep folder names concise (1-3 words). Use the file metadata, snippets,\n\
and folder context to make informed grouping decisions.\n\
Use \"uncategorized\" only when evidence is genuinely ambiguous.\n\
\n\
If a file's current name is cryptic, generic, or auto-generated\n\
(e.g., \"IMG_20240315.jpg\", \"Document (3).pdf\", \"Screenshot 2024-01-15 at 10.23.45.png\"),\n\
suggest a descriptive name in \"suggested_name\". Always preserve the original file extension.\n\
Omit \"suggested_name\" when the current name is already descriptive.\n\
\n\
Global tree summary JSON:\n\
{summary_json}\n\
\n\
Chunk file manifest JSON (all paths are relative to root):\n\
{json_files}\n\
\n\
Rules:\n\
1. Assign every item in this chunk to exactly one folder.\n\
2. Prefer path and filename semantics, then extension/mime, then snippet.\n\
3. Keep sibling and parent-folder cohorts consistent where possible.\n\
4. Return ONLY a JSON code block with this schema:\n\
```json\n\
{{\n\
  \"assignments\": [\n\
    {{ \"i\": 0, \"folder\": \"tax_returns\", \"subfolder\": \"2024\" }},\n\
    {{ \"i\": 1, \"folder\": \"vacation_photos\", \"suggested_name\": \"beach_sunset.jpg\" }}\n\
  ]\n\
}}\n\
```\n\
No other text."
    ))
}

pub fn build_refinement_chunk_prompt(
    root_path: &str,
    chunk_index: usize,
    chunk_total: usize,
    tree_summary: &serde_json::Value,
    chunk_files: &[IndexedManifestFile],
    current_assignments: &HashMap<String, String>,
    suggestions: &HashMap<String, DeterministicClassification>,
) -> Result<String, AppError> {
    let manifest_payload = chunk_files
        .iter()
        .enumerate()
        .map(|file| {
            let (i, file) = file;
            let current_category = current_assignments
                .get(&file.relative_path)
                .cloned()
                .unwrap_or_else(|| "other".to_string());
            let suggestion = suggestions.get(&file.relative_path);
            serde_json::json!({
                "i": i,
                "relative_path": file.relative_path,
                "parent_relative": file.parent_relative,
                "extension": file.extension,
                "mime_type": file.mime_type,
                "snippet": file.snippet,
                "current_category": current_category,
                "type_hint": suggestion.map(|s| s.type_hint.hint_label()),
            })
        })
        .collect::<Vec<_>>();
    let json_files = serde_json::to_string_pretty(&manifest_payload)?;
    let summary_json = serde_json::to_string_pretty(tree_summary)?;

    Ok(format!(
        "You are refining uncertain file classifications for a file tree.\n\
Root path: {root_path}\n\
Refinement chunk: {chunk_index}/{chunk_total}\n\
\n\
Suggest descriptive, content-based folder names using snake_case.\n\
Group related files together. Choose names that describe the content,\n\
not just the file type. Keep folder names concise (1-3 words).\n\
\n\
If a file's current name is cryptic, generic, or auto-generated\n\
(e.g., \"IMG_20240315.jpg\", \"Document (3).pdf\", \"Screenshot 2024-01-15 at 10.23.45.png\"),\n\
suggest a descriptive name in \"suggested_name\". Always preserve the original file extension.\n\
Omit \"suggested_name\" when the current name is already descriptive.\n\
\n\
Global tree summary JSON:\n\
{summary_json}\n\
\n\
Refinement file manifest JSON (all paths are relative to root):\n\
{json_files}\n\
\n\
Rules:\n\
1. Assign every item to exactly one folder.\n\
2. Move files out of \"uncategorized\" when there is coherent evidence.\n\
3. Keep cohort consistency with sibling/parent signals.\n\
4. Return ONLY a JSON code block with this schema:\n\
```json\n\
{{\n\
  \"assignments\": [\n\
    {{ \"i\": 0, \"folder\": \"tax_returns\", \"subfolder\": \"2024\" }},\n\
    {{ \"i\": 1, \"folder\": \"vacation_photos\", \"suggested_name\": \"beach_sunset.jpg\" }}\n\
  ]\n\
}}\n\
```\n\
No other text."
    ))
}

pub fn extract_json_payload(text: &str) -> Option<String> {
    let trimmed = text.trim();

    if let Some(start) = trimmed.find("```json") {
        let rest = &trimmed[start + "```json".len()..];
        if let Some(end) = rest.find("```") {
            return Some(rest[..end].trim().to_string());
        }
    }

    if let Some(start) = trimmed.find("```") {
        let rest = &trimmed[start + 3..];
        if let Some(newline) = rest.find('\n') {
            let body = &rest[newline + 1..];
            if let Some(end) = body.find("```") {
                return Some(body[..end].trim().to_string());
            }
        }
    }

    let first = trimmed.find('{')?;
    let last = trimmed.rfind('}')?;
    if first <= last {
        return Some(trimmed[first..=last].to_string());
    }

    None
}

pub fn parse_chunk_plan(
    text: &str,
    chunk_files: &[IndexedManifestFile],
    deterministic: &HashMap<String, DeterministicClassification>,
) -> Result<ParsedChunkPlan, AppError> {
    let payload = extract_json_payload(text).ok_or_else(|| {
        AppError::General("Model response did not contain a valid JSON payload".to_string())
    })?;
    let parsed: OrganizeChunkPayload = serde_json::from_str(&payload)?;

    let chunk_rel_paths: HashSet<String> = chunk_files
        .iter()
        .map(|file| normalize_relative_path(&file.relative_path))
        .collect();
    let index_lookup: HashMap<usize, String> = chunk_files
        .iter()
        .enumerate()
        .map(|(idx, file)| (idx, normalize_relative_path(&file.relative_path)))
        .collect();

    let mut per_category: HashMap<String, Vec<String>> = HashMap::new();
    let mut subfolders: HashMap<String, Option<String>> = HashMap::new();
    let mut suggested_names: HashMap<String, Option<String>> = HashMap::new();
    let mut fallback_paths: HashSet<String> = HashSet::new();
    let mut assigned: HashSet<String> = HashSet::new();

    let mut record = |rel: String,
                      folder: String,
                      subfolder: Option<String>,
                      suggested_name: Option<String>,
                      used_fallback: bool| {
        if rel.is_empty() || !chunk_rel_paths.contains(&rel) || assigned.contains(&rel) {
            return;
        }
        per_category
            .entry(folder.clone())
            .or_default()
            .push(rel.clone());
        subfolders.insert(
            rel.clone(),
            subfolder.and_then(|value| validate_subfolder(&value)),
        );
        let validated_name = suggested_name
            .as_deref()
            .and_then(|name| validate_suggested_name(name, &rel));
        suggested_names.insert(rel.clone(), validated_name);
        if used_fallback {
            fallback_paths.insert(rel.clone());
        }
        assigned.insert(rel);
    };

    for assignment in parsed.assignments {
        let maybe_rel = assignment
            .i
            .and_then(|idx| index_lookup.get(&idx).cloned())
            .or_else(|| assignment.file.as_deref().map(normalize_relative_path));
        let Some(rel) = maybe_rel else {
            continue;
        };
        record(
            rel,
            validate_folder_name(&assignment.folder),
            assignment.subfolder,
            assignment.suggested_name,
            false,
        );
    }

    for category in parsed.categories {
        let folder = validate_folder_name(&category.folder);
        for file in category.files {
            let rel = normalize_relative_path(&file);
            record(rel, folder.clone(), None, None, false);
        }
    }

    for rel in chunk_files
        .iter()
        .map(|file| normalize_relative_path(&file.relative_path))
    {
        let (folder, subfolder) = fallback_category_for_path(&rel, deterministic);
        record(rel, folder, subfolder, None, true);
    }

    Ok(ParsedChunkPlan {
        category_map: per_category,
        subfolders,
        suggested_names,
        fallback_paths,
    })
}

pub fn fallback_chunk_plan(
    chunk_files: &[IndexedManifestFile],
    deterministic: &HashMap<String, DeterministicClassification>,
) -> ParsedChunkPlan {
    let mut category_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut subfolders: HashMap<String, Option<String>> = HashMap::new();
    let mut suggested_names: HashMap<String, Option<String>> = HashMap::new();
    let mut fallback_paths: HashSet<String> = HashSet::new();

    for file in chunk_files {
        let rel = normalize_relative_path(&file.relative_path);
        if rel.is_empty() {
            continue;
        }
        let (folder, subfolder) = fallback_category_for_path(&rel, deterministic);
        category_map.entry(folder).or_default().push(rel.clone());
        subfolders.insert(rel.clone(), subfolder);
        suggested_names.insert(rel.clone(), None);
        fallback_paths.insert(rel);
    }

    ParsedChunkPlan {
        category_map,
        subfolders,
        suggested_names,
        fallback_paths,
    }
}

fn is_unclassified(folder: &str) -> bool {
    folder == "other" || folder == "uncategorized"
}

fn assign_path(assignments: &mut HashMap<String, String>, rel_path: String, folder: String) {
    if rel_path.is_empty() {
        return;
    }

    match assignments.get(&rel_path) {
        Some(existing) if !is_unclassified(existing) && is_unclassified(&folder) => {}
        Some(existing) if !is_unclassified(existing) && !is_unclassified(&folder) => {}
        Some(existing) if is_unclassified(existing) && is_unclassified(&folder) => {}
        Some(_) => {
            assignments.insert(rel_path, folder);
        }
        None => {
            assignments.insert(rel_path, folder);
        }
    }
}

pub fn category_assignments(
    category_map: &HashMap<String, Vec<String>>,
) -> HashMap<String, String> {
    let mut assignments = HashMap::new();

    for (folder, files) in category_map {
        let validated = validate_folder_name(folder);
        for rel in files {
            let normalized = normalize_path(rel.trim());
            assign_path(&mut assignments, normalized, validated.clone());
        }
    }

    assignments
}

pub fn assignments_to_category_map(
    assignments: &HashMap<String, String>,
) -> HashMap<String, Vec<String>> {
    let mut merged: HashMap<String, Vec<String>> = HashMap::new();

    for (path, folder) in assignments {
        let normalized_path = normalize_path(path.trim());
        if normalized_path.is_empty() {
            continue;
        }
        let validated = validate_folder_name(folder);
        merged.entry(validated).or_default().push(normalized_path);
    }

    for files in merged.values_mut() {
        files.sort();
        files.dedup();
    }

    merged
}

#[cfg(test)]
pub fn reconcile_with_refinement(
    initial: HashMap<String, Vec<String>>,
    refinement: HashMap<String, Vec<String>>,
) -> HashMap<String, Vec<String>> {
    let mut assignments = category_assignments(&initial);
    let refinement_assignments = category_assignments(&refinement);

    for (path, folder) in refinement_assignments {
        if is_unclassified(&folder) {
            assignments.entry(path).or_insert(folder);
            continue;
        }
        assignments.insert(path, folder);
    }

    assignments_to_category_map(&assignments)
}

pub fn build_plan_document(
    merged_categories: HashMap<String, Vec<String>>,
    mut placements: Vec<OrganizePlacement>,
    stats: OrganizePlanStats,
) -> OrganizePlanDocument {
    let mut categories = Vec::new();

    let mut folder_names: Vec<&String> = merged_categories.keys().collect();
    folder_names.sort();

    for folder in folder_names {
        if folder == "other" || folder == "uncategorized" {
            continue;
        }
        if let Some(mut files) = merged_categories.get(folder).cloned() {
            files.sort();
            files.dedup();
            if files.is_empty() {
                continue;
            }
            categories.push(OrganizeCategory {
                folder: folder.clone(),
                description: format!("{folder} files"),
                files,
            });
        }
    }

    let mut unclassified_set = BTreeSet::new();
    if let Some(files) = merged_categories.get("other") {
        unclassified_set.extend(files.iter().cloned());
    }
    if let Some(files) = merged_categories.get("uncategorized") {
        unclassified_set.extend(files.iter().cloned());
    }
    let unclassified = unclassified_set.into_iter().collect::<Vec<_>>();

    placements.sort_by(|a, b| a.path.cmp(&b.path));
    placements.dedup_by(|a, b| a.path == b.path);

    OrganizePlanDocument {
        taxonomy_version: "v3".to_string(),
        categories,
        placements,
        unclassified,
        stats,
    }
}

pub fn plan_to_json_block(plan: &OrganizePlanDocument) -> Result<String, AppError> {
    let pretty = serde_json::to_string_pretty(plan)?;
    Ok(format!("```json\n{pretty}\n```"))
}

fn join_normalized(base: &str, tail: &str) -> String {
    if base == "/" {
        format!("/{tail}")
    } else {
        format!("{base}/{tail}")
    }
}

fn unique_destination(
    destination: &str,
    planned_sources: &HashSet<String>,
    reserved_destinations: &HashSet<String>,
    existing_paths: &HashSet<String>,
) -> Option<String> {
    if destination.is_empty() {
        return None;
    }

    if reserved_destinations.contains(destination) {
        return None;
    }

    if existing_paths.contains(destination) && !planned_sources.contains(destination) {
        return None;
    }

    if Path::new(destination).exists() && !planned_sources.contains(destination) {
        return None;
    }

    Some(destination.to_string())
}

pub fn build_action_batch(
    root_path: &str,
    plan: &OrganizePlan,
    existing_absolute_paths: &HashSet<String>,
) -> OrganizeActionBatch {
    let root = normalize_path(root_path);
    let mut warnings = Vec::new();
    let mut create_dirs: BTreeSet<String> = BTreeSet::new();
    let mut moves_by_dest: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut reserved_destinations: HashSet<String> = HashSet::new();
    let mut renames: Vec<(String, String)> = Vec::new();

    let planned = if !plan.placements.is_empty() {
        plan.placements
            .iter()
            .map(|placement| {
                (
                    normalize_relative_path(&placement.path),
                    validate_folder_name(&placement.folder),
                    placement.subfolder.as_deref().and_then(validate_subfolder),
                    placement
                        .packing_path
                        .as_deref()
                        .and_then(normalize_packing_path),
                    placement.suggested_name.clone(),
                )
            })
            .collect::<Vec<_>>()
    } else {
        plan.categories
            .iter()
            .flat_map(|category| {
                let folder = validate_folder_name(&category.folder);
                category.files.iter().map(move |rel| {
                    (
                        normalize_relative_path(rel),
                        folder.clone(),
                        None::<String>,
                        None::<String>,
                        None::<String>,
                    )
                })
            })
            .collect::<Vec<_>>()
    };

    let mut planned_sources: HashSet<String> = HashSet::new();
    for (relative, _, _, _, _) in &planned {
        if relative.is_empty() {
            continue;
        }
        planned_sources.insert(join_normalized(&root, relative));
    }

    for (relative, folder, subfolder, packing_path, suggested_name) in planned {
        if relative.is_empty() {
            continue;
        }

        let source_abs = join_normalized(&root, &relative);
        if !existing_absolute_paths.contains(&source_abs) {
            warnings.push(format!("Missing source in index, skipped: {source_abs}"));
            continue;
        }

        let file_name = Path::new(&relative)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        if file_name.is_empty() {
            warnings.push(format!("Invalid source path, skipped: {source_abs}"));
            continue;
        }

        let parent_relative = Path::new(&relative)
            .parent()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let mut folder_prefix = folder;
        if let Some(subfolder) = subfolder {
            folder_prefix = format!("{folder_prefix}/{subfolder}");
        }
        if let Some(packing_path) = packing_path {
            folder_prefix = format!("{folder_prefix}/{packing_path}");
        }
        let dest_dir = if parent_relative.is_empty() {
            join_normalized(&root, &folder_prefix)
        } else {
            join_normalized(&root, &format!("{folder_prefix}/{parent_relative}"))
        };
        let destination_abs = join_normalized(&dest_dir, &file_name);

        if source_abs == destination_abs && suggested_name.is_none() {
            continue;
        }

        if source_abs != destination_abs {
            let Some(_safe_destination) = unique_destination(
                &destination_abs,
                &planned_sources,
                &reserved_destinations,
                existing_absolute_paths,
            ) else {
                warnings.push(format!(
                    "Destination collision, skipped move: {source_abs} -> {destination_abs}"
                ));
                continue;
            };

            create_dirs.insert(dest_dir.clone());
            moves_by_dest
                .entry(dest_dir.clone())
                .or_default()
                .push(source_abs);
            reserved_destinations.insert(destination_abs.clone());
        }

        if let Some(new_name) = suggested_name {
            if new_name != file_name {
                let rename_source = destination_abs;
                let rename_dest = join_normalized(&dest_dir, &new_name);
                if unique_destination(
                    &rename_dest,
                    &planned_sources,
                    &reserved_destinations,
                    existing_absolute_paths,
                )
                .is_some()
                {
                    renames.push((rename_source, rename_dest.clone()));
                    reserved_destinations.insert(rename_dest);
                } else {
                    warnings.push(format!(
                        "Rename collision, skipped: {rename_source} -> {rename_dest}"
                    ));
                }
            }
        }
    }

    let mut actions = Vec::new();

    for path in create_dirs {
        actions.push(OrganizeAction {
            tool: "create_directory".to_string(),
            args: serde_json::json!({ "path": path }),
        });
    }

    for (dest_dir, mut sources) in moves_by_dest {
        sources.sort();
        sources.dedup();
        if sources.is_empty() {
            continue;
        }
        actions.push(OrganizeAction {
            tool: "move_files".to_string(),
            args: serde_json::json!({ "sources": sources, "dest_dir": dest_dir }),
        });
    }

    for (source, destination) in &renames {
        actions.push(OrganizeAction {
            tool: "rename_file".to_string(),
            args: serde_json::json!({ "source": source, "destination": destination }),
        });
    }

    OrganizeActionBatch { actions, warnings }
}

pub fn actions_to_blocks(batch: &OrganizeActionBatch) -> Result<String, AppError> {
    if batch.actions.is_empty() {
        let warning_text = if batch.warnings.is_empty() {
            "No safe actions were generated.".to_string()
        } else {
            batch.warnings.join("\n")
        };
        return Err(AppError::General(warning_text));
    }

    let mut out = String::new();

    if !batch.warnings.is_empty() {
        out.push_str("Warnings:\n");
        for warning in &batch.warnings {
            out.push_str("- ");
            out.push_str(warning);
            out.push('\n');
        }
        out.push('\n');
    }

    for action in &batch.actions {
        let payload = serde_json::json!({
            "tool": action.tool,
            "args": action.args,
        });
        out.push_str("```action\n");
        out.push_str(&serde_json::to_string(&payload)?);
        out.push_str("\n```\n");
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_hint_from_extension_classifies_common_types() {
        assert_eq!(type_hint_from_extension("jpg"), Some(FileTypeHint::Image));
        assert_eq!(
            type_hint_from_extension("pdf"),
            Some(FileTypeHint::Document)
        );
        assert_eq!(type_hint_from_extension("rs"), Some(FileTypeHint::Code));
        assert_eq!(
            type_hint_from_extension("csv"),
            Some(FileTypeHint::Spreadsheet)
        );
        assert_eq!(type_hint_from_extension("mp4"), Some(FileTypeHint::Video));
        assert_eq!(type_hint_from_extension("mp3"), Some(FileTypeHint::Audio));
        assert_eq!(type_hint_from_extension("zip"), Some(FileTypeHint::Archive));
        assert_eq!(type_hint_from_extension("json"), Some(FileTypeHint::Data));
        assert_eq!(type_hint_from_extension("psd"), Some(FileTypeHint::Design));
        assert_eq!(type_hint_from_extension("xyz"), None);
    }

    #[test]
    fn extract_json_payload_handles_codeblock() {
        let text = "hello\n```json\n{\"categories\": []}\n```\n";
        let payload = extract_json_payload(text).unwrap();
        assert_eq!(payload, "{\"categories\": []}");
    }

    #[test]
    fn parse_chunk_plan_assigns_unclassified_to_other() {
        let text = "```json\n{\"categories\":[{\"folder\":\"projects\",\"description\":\"x\",\"files\":[\"a/b.txt\"]}]}\n```";
        let chunk = vec![
            IndexedManifestFile {
                absolute_path: "/root/a/b.txt".to_string(),
                relative_path: "a/b.txt".to_string(),
                parent_relative: "a".to_string(),
                depth: 1,
                extension: Some("txt".to_string()),
                mime_type: Some("text/plain".to_string()),
                size_bytes: Some(10),
                modified_at: None,
                snippet: None,
            },
            IndexedManifestFile {
                absolute_path: "/root/c.txt".to_string(),
                relative_path: "c.txt".to_string(),
                parent_relative: "".to_string(),
                depth: 0,
                extension: Some("txt".to_string()),
                mime_type: Some("text/plain".to_string()),
                size_bytes: Some(10),
                modified_at: None,
                snippet: None,
            },
        ];
        let deterministic = classify_manifest_deterministic(&chunk);
        let parsed = parse_chunk_plan(text, &chunk, &deterministic).unwrap();
        assert_eq!(
            parsed.category_map.get("projects").unwrap(),
            &vec!["a/b.txt".to_string()]
        );
        assert_eq!(
            parsed.category_map.get("documents").unwrap(),
            &vec!["c.txt".to_string()]
        );
        assert!(parsed.fallback_paths.contains("c.txt"));
    }

    #[test]
    fn build_action_batch_preserves_relative_tree() {
        let plan = OrganizePlan {
            categories: vec![OrganizeCategory {
                folder: "projects".to_string(),
                description: "x".to_string(),
                files: vec!["foo/bar.txt".to_string()],
            }],
            placements: Vec::new(),
        };
        let mut existing = HashSet::new();
        existing.insert("/root/foo/bar.txt".to_string());

        let batch = build_action_batch("/root", &plan, &existing);
        assert_eq!(batch.actions.len(), 2);
        assert_eq!(batch.actions[0].tool, "create_directory");
        assert_eq!(batch.actions[1].tool, "move_files");
    }

    #[test]
    fn category_assignments_prefers_non_other() {
        let mut map = HashMap::new();
        map.insert(
            "other".to_string(),
            vec!["src/main.rs".to_string(), "docs/readme.md".to_string()],
        );
        map.insert(
            "projects".to_string(),
            vec!["src/main.rs".to_string(), "src/lib.rs".to_string()],
        );

        let assignments = category_assignments(&map);

        assert_eq!(assignments.get("src/main.rs").unwrap(), "projects");
        assert_eq!(assignments.get("src/lib.rs").unwrap(), "projects");
        assert_eq!(assignments.get("docs/readme.md").unwrap(), "other");
    }

    #[test]
    fn reconcile_with_refinement_promotes_other_files() {
        let mut initial = HashMap::new();
        initial.insert(
            "other".to_string(),
            vec!["a.txt".to_string(), "b.txt".to_string()],
        );
        initial.insert("projects".to_string(), vec!["c.rs".to_string()]);

        let mut refinement = HashMap::new();
        refinement.insert("documents".to_string(), vec!["a.txt".to_string()]);
        refinement.insert("other".to_string(), vec!["b.txt".to_string()]);

        let reconciled = reconcile_with_refinement(initial, refinement);

        assert_eq!(
            reconciled.get("documents").unwrap(),
            &vec!["a.txt".to_string()]
        );
        assert_eq!(reconciled.get("other").unwrap(), &vec!["b.txt".to_string()]);
        assert_eq!(
            reconciled.get("projects").unwrap(),
            &vec!["c.rs".to_string()]
        );
    }

    #[test]
    fn build_refinement_chunk_prompt_includes_current_category() {
        let files = vec![IndexedManifestFile {
            absolute_path: "/root/a.txt".to_string(),
            relative_path: "a.txt".to_string(),
            parent_relative: "".to_string(),
            depth: 0,
            extension: Some("txt".to_string()),
            mime_type: Some("text/plain".to_string()),
            size_bytes: Some(3),
            modified_at: None,
            snippet: Some("abc".to_string()),
        }];
        let mut assignments = HashMap::new();
        assignments.insert("a.txt".to_string(), "other".to_string());

        let prompt = build_refinement_chunk_prompt(
            "/root",
            1,
            1,
            &serde_json::json!({"top_level_counts": {}, "extension_counts": {}}),
            &files,
            &assignments,
            &HashMap::new(),
        )
        .unwrap();

        assert!(prompt.contains("\"current_category\": \"other\""));
        assert!(prompt.contains("Refinement chunk: 1/1"));
    }

    #[test]
    fn deterministic_classifier_detects_finance_documents() {
        let file = IndexedManifestFile {
            absolute_path: "/root/invoices/ada_invoice_20457.pdf".to_string(),
            relative_path: "invoices/ada_invoice_20457.pdf".to_string(),
            parent_relative: "invoices".to_string(),
            depth: 1,
            extension: Some("pdf".to_string()),
            mime_type: Some("application/pdf".to_string()),
            size_bytes: Some(1234),
            modified_at: None,
            snippet: Some("Invoice total due amount subtotal tax".to_string()),
        };

        let decision = classify_file_deterministic(&file);
        assert_eq!(decision.type_hint, FileTypeHint::Document);
        assert!(decision.confidence > 0.0);
    }

    #[test]
    fn build_action_batch_uses_placements_subfolder() {
        let plan = OrganizePlan {
            categories: Vec::new(),
            placements: vec![OrganizePlacement {
                path: "contracts/msa.pdf".to_string(),
                folder: "documents".to_string(),
                subfolder: Some("legal".to_string()),
                packing_path: None,
                suggested_name: None,
                confidence: 0.9,
                source: "llm".to_string(),
            }],
        };
        let mut existing = HashSet::new();
        existing.insert("/root/contracts/msa.pdf".to_string());

        let batch = build_action_batch("/root", &plan, &existing);
        let create = batch
            .actions
            .iter()
            .find(|action| action.tool == "create_directory")
            .unwrap();
        assert_eq!(
            create.args["path"].as_str().unwrap(),
            "/root/documents/legal/contracts"
        );
    }

    #[test]
    fn build_action_batch_uses_packing_path() {
        let plan = OrganizePlan {
            categories: Vec::new(),
            placements: vec![OrganizePlacement {
                path: "invoices/2025/acme_001.pdf".to_string(),
                folder: "documents".to_string(),
                subfolder: Some("finance".to_string()),
                packing_path: Some("acme_2025_01_pdf_s01".to_string()),
                suggested_name: None,
                confidence: 0.9,
                source: "deterministic".to_string(),
            }],
        };
        let mut existing = HashSet::new();
        existing.insert("/root/invoices/2025/acme_001.pdf".to_string());

        let batch = build_action_batch("/root", &plan, &existing);
        let create = batch
            .actions
            .iter()
            .find(|action| action.tool == "create_directory")
            .unwrap();
        assert_eq!(
            create.args["path"].as_str().unwrap(),
            "/root/documents/finance/acme_2025_01_pdf_s01/invoices/2025"
        );
    }

    #[test]
    fn capacity_packing_splits_large_bucket() {
        let mut placements = Vec::new();
        let mut manifest = Vec::new();
        for idx in 0..75 {
            let path = format!("finance/invoice_{idx:03}.pdf");
            placements.push(OrganizePlacement {
                path: path.clone(),
                folder: "documents".to_string(),
                subfolder: Some("finance".to_string()),
                packing_path: None,
                suggested_name: None,
                confidence: 0.9,
                source: "deterministic".to_string(),
            });
            manifest.push(IndexedManifestFile {
                absolute_path: format!("/root/{path}"),
                relative_path: path,
                parent_relative: "finance".to_string(),
                depth: 1,
                extension: Some("pdf".to_string()),
                mime_type: Some("application/pdf".to_string()),
                size_bytes: Some(100),
                modified_at: Some("2025-01-01T00:00:00Z".to_string()),
                snippet: Some("invoice total".to_string()),
            });
        }

        let (packed, stats) = apply_capacity_packing(placements, &manifest);
        assert!(stats.packed_directories >= 2);
        assert!(stats.max_children_observed <= PACKING_MAX_TARGET);
        assert!(packed.iter().all(|item| item.packing_path.is_some()));
    }

    #[test]
    fn capacity_packing_splits_bucket_above_target() {
        let mut placements = Vec::new();
        let mut manifest = Vec::new();
        for idx in 0..80 {
            let month = (idx % 3) + 1;
            let path = format!("finance/statement_{idx:03}.pdf");
            placements.push(OrganizePlacement {
                path: path.clone(),
                folder: "documents".to_string(),
                subfolder: Some("finance".to_string()),
                packing_path: None,
                suggested_name: None,
                confidence: 0.9,
                source: "deterministic".to_string(),
            });
            manifest.push(IndexedManifestFile {
                absolute_path: format!("/root/{path}"),
                relative_path: path,
                parent_relative: "finance".to_string(),
                depth: 1,
                extension: Some("pdf".to_string()),
                mime_type: Some("application/pdf".to_string()),
                size_bytes: Some(100),
                modified_at: Some(format!("2025-{month:02}-01T00:00:00Z")),
                snippet: Some("monthly statement".to_string()),
            });
        }

        let (packed, stats) = apply_capacity_packing(placements, &manifest);
        let mut counts: HashMap<String, usize> = HashMap::new();
        for item in &packed {
            if let Some(packing_path) = item.packing_path.as_deref() {
                *counts.entry(packing_path.to_string()).or_default() += 1;
            }
        }

        assert!(counts.len() >= 2);
        assert!(counts.values().copied().max().unwrap_or(0) <= PACKING_MAX_TARGET);
        assert_eq!(stats.folders_over_hard_max, 0);
        assert!(stats.max_children_observed <= PACKING_MAX_TARGET);
    }

    #[test]
    fn capacity_packing_limits_depth_to_policy_max() {
        let mut placements = Vec::new();
        let mut manifest = Vec::new();
        for idx in 0..420 {
            let vendor = idx % 9;
            let month = (idx % 12) + 1;
            let path = format!("finance/vendor_{vendor}/invoice_2025_{month:02}_{idx:03}.pdf");
            placements.push(OrganizePlacement {
                path: path.clone(),
                folder: "documents".to_string(),
                subfolder: Some("finance".to_string()),
                packing_path: None,
                suggested_name: None,
                confidence: 0.8,
                source: "deterministic".to_string(),
            });
            manifest.push(IndexedManifestFile {
                absolute_path: format!("/root/{path}"),
                relative_path: path,
                parent_relative: format!("finance/vendor_{vendor}"),
                depth: 2,
                extension: Some("pdf".to_string()),
                mime_type: Some("application/pdf".to_string()),
                size_bytes: Some(200),
                modified_at: Some(format!("2025-{month:02}-15T00:00:00Z")),
                snippet: Some("invoice amount due".to_string()),
            });
        }

        let (packed, stats) = apply_capacity_packing(placements, &manifest);
        assert!(packed.iter().all(|item| {
            item.packing_path
                .as_deref()
                .map(|path| path.split('/').count() <= PACKING_MAX_DEPTH)
                .unwrap_or(false)
        }));
        assert_eq!(stats.folders_over_hard_max, 0);
    }

    #[test]
    fn validate_folder_name_accepts_descriptive_names() {
        assert_eq!(validate_folder_name("tax_returns"), "tax_returns");
        assert_eq!(
            validate_folder_name("Vacation Photos 2024"),
            "vacation_photos_2024"
        );
        assert_eq!(validate_folder_name("website-source"), "website_source");
        assert_eq!(validate_folder_name("   projects   "), "projects");
    }

    #[test]
    fn validate_folder_name_rejects_bad_input() {
        assert_eq!(validate_folder_name(""), "uncategorized");
        assert_eq!(validate_folder_name("   "), "uncategorized");
        assert_eq!(validate_folder_name("..."), "uncategorized");
    }

    #[test]
    fn validate_folder_name_truncates_long_names() {
        let long = "a".repeat(60);
        let result = validate_folder_name(&long);
        assert_eq!(result.len(), 50);
    }

    #[test]
    fn validate_suggested_name_preserves_extension() {
        assert_eq!(
            validate_suggested_name("beach_sunset", "IMG_20240315.jpg"),
            Some("beach_sunset.jpg".to_string())
        );
        assert_eq!(
            validate_suggested_name("beach_sunset.png", "IMG_20240315.jpg"),
            Some("beach_sunset.jpg".to_string())
        );
    }

    #[test]
    fn validate_suggested_name_rejects_bad_input() {
        assert_eq!(validate_suggested_name("", "photo.jpg"), None);
        assert_eq!(validate_suggested_name("../etc/passwd", "photo.jpg"), None);
        assert_eq!(validate_suggested_name("/root/bad", "photo.jpg"), None);
        assert_eq!(validate_suggested_name(".hidden", "photo.jpg"), None);
    }

    #[test]
    fn validate_suggested_name_skips_unchanged() {
        assert_eq!(validate_suggested_name("photo.jpg", "photo.jpg"), None);
    }

    #[test]
    fn build_action_batch_generates_rename_actions() {
        let plan = OrganizePlan {
            categories: Vec::new(),
            placements: vec![OrganizePlacement {
                path: "IMG_20240315.jpg".to_string(),
                folder: "vacation_photos".to_string(),
                subfolder: None,
                packing_path: None,
                suggested_name: Some("beach_sunset.jpg".to_string()),
                confidence: 0.9,
                source: "llm".to_string(),
            }],
        };
        let mut existing = HashSet::new();
        existing.insert("/root/IMG_20240315.jpg".to_string());

        let batch = build_action_batch("/root", &plan, &existing);
        assert_eq!(batch.actions.len(), 3);
        assert_eq!(batch.actions[0].tool, "create_directory");
        assert_eq!(batch.actions[1].tool, "move_files");
        assert_eq!(batch.actions[2].tool, "rename_file");
        assert_eq!(
            batch.actions[2].args["source"].as_str().unwrap(),
            "/root/vacation_photos/IMG_20240315.jpg"
        );
        assert_eq!(
            batch.actions[2].args["destination"].as_str().unwrap(),
            "/root/vacation_photos/beach_sunset.jpg"
        );
    }

    #[test]
    fn build_action_batch_skips_rename_when_name_unchanged() {
        let plan = OrganizePlan {
            categories: Vec::new(),
            placements: vec![OrganizePlacement {
                path: "report.pdf".to_string(),
                folder: "tax_returns".to_string(),
                subfolder: None,
                packing_path: None,
                suggested_name: Some("report.pdf".to_string()),
                confidence: 0.9,
                source: "llm".to_string(),
            }],
        };
        let mut existing = HashSet::new();
        existing.insert("/root/report.pdf".to_string());

        let batch = build_action_batch("/root", &plan, &existing);
        let rename_count = batch
            .actions
            .iter()
            .filter(|a| a.tool == "rename_file")
            .count();
        assert_eq!(rename_count, 0);
    }

    #[test]
    fn parse_chunk_plan_extracts_suggested_name() {
        let text = r#"```json
{"assignments":[{"i":0,"folder":"vacation_photos","suggested_name":"beach_sunset.jpg"}]}
```"#;
        let chunk = vec![IndexedManifestFile {
            absolute_path: "/root/IMG_001.jpg".to_string(),
            relative_path: "IMG_001.jpg".to_string(),
            parent_relative: "".to_string(),
            depth: 0,
            extension: Some("jpg".to_string()),
            mime_type: Some("image/jpeg".to_string()),
            size_bytes: Some(100),
            modified_at: None,
            snippet: None,
        }];
        let deterministic = classify_manifest_deterministic(&chunk);
        let parsed = parse_chunk_plan(text, &chunk, &deterministic).unwrap();
        assert!(parsed.category_map.contains_key("vacation_photos"));
        assert_eq!(
            parsed
                .suggested_names
                .get("IMG_001.jpg")
                .unwrap()
                .as_deref(),
            Some("beach_sunset.jpg")
        );
    }

    #[test]
    fn category_assignments_works_with_freeform_folders() {
        let mut map = HashMap::new();
        map.insert("tax_returns".to_string(), vec!["invoice.pdf".to_string()]);
        map.insert("vacation_photos".to_string(), vec!["beach.jpg".to_string()]);

        let assignments = category_assignments(&map);
        assert_eq!(assignments.get("invoice.pdf").unwrap(), "tax_returns");
        assert_eq!(assignments.get("beach.jpg").unwrap(), "vacation_photos");
    }

    #[test]
    fn build_plan_document_sorts_freeform_folders_alphabetically() {
        let mut categories = HashMap::new();
        categories.insert("zebra_files".to_string(), vec!["z.txt".to_string()]);
        categories.insert("alpha_docs".to_string(), vec!["a.txt".to_string()]);
        categories.insert("other".to_string(), vec!["unknown.bin".to_string()]);

        let doc = build_plan_document(
            categories,
            Vec::new(),
            OrganizePlanStats {
                total_files: 3,
                indexed_files: 3,
                skipped_hidden: 0,
                skipped_already_organized: 0,
                chunks: 1,
                other_count: 1,
                other_ratio: 0.33,
                deterministic_assigned: 0,
                fast_classified: 0,
                llm_assigned: 3,
                fallback_assigned: 0,
                parse_failed_chunks: 0,
                packed_directories: 0,
                max_children_observed: 0,
                avg_children_per_generated_dir: 0.0,
                capacity_overflow_dirs: 0,
                packing_llm_calls: 0,
                folders_over_target: 0,
                folders_over_hard_max: 0,
                avg_depth_generated: 0.0,
                fallback_label_rate: 0.0,
            },
        );

        assert_eq!(doc.categories.len(), 2);
        assert_eq!(doc.categories[0].folder, "alpha_docs");
        assert_eq!(doc.categories[1].folder, "zebra_files");
        assert_eq!(doc.unclassified, vec!["unknown.bin".to_string()]);
    }

    fn make_file(relative_path: &str, parent: &str, ext: &str, mime: &str) -> IndexedManifestFile {
        IndexedManifestFile {
            absolute_path: format!("/root/{relative_path}"),
            relative_path: relative_path.to_string(),
            parent_relative: parent.to_string(),
            depth: relative_path.matches('/').count(),
            extension: Some(ext.to_string()),
            mime_type: Some(mime.to_string()),
            size_bytes: Some(1024),
            modified_at: None,
            snippet: None,
        }
    }

    #[test]
    fn fast_classify_assigns_image_with_strong_signal() {
        let file = make_file("photos/sunset.jpg", "photos", "jpg", "image/jpeg");
        let result = classify_file_fast(&file);
        assert!(
            result.is_some(),
            "jpg with image/ mime should fast-classify"
        );
        let fc = result.unwrap();
        assert_eq!(fc.folder, "photos");
    }

    #[test]
    fn fast_classify_skips_documents() {
        let file = make_file("reports/budget.pdf", "reports", "pdf", "application/pdf");
        let result = classify_file_fast(&file);
        assert!(
            result.is_none(),
            "pdf documents should NOT be fast-classified"
        );
    }

    #[test]
    fn fast_classify_uses_descriptive_parent_dir() {
        // "video" in parent triggers Video keyword (+0.45), ext (+0.25), mime (+0.25) = 0.95
        let file = make_file(
            "family_video/birthday.mp4",
            "family_video",
            "mp4",
            "video/mp4",
        );
        let result = classify_file_fast(&file);
        assert!(result.is_some());
        assert_eq!(result.unwrap().folder, "family_video");
    }

    #[test]
    fn fast_classify_uses_default_for_generic_parent() {
        // "music" in path triggers Audio keyword (+0.45), ext (+0.25), mime (+0.25) = 0.95
        let file = make_file(
            "Downloads/music_collection/track.mp3",
            "Downloads",
            "mp3",
            "audio/mpeg",
        );
        let result = classify_file_fast(&file);
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().folder,
            "audio",
            "generic parent 'Downloads' should fall back to type default"
        );
    }

    #[test]
    fn fast_classify_skips_low_confidence() {
        // A file with an unknown extension and no mime -- no strong signal
        let file = make_file("stuff/mystery.xyz", "stuff", "xyz", "");
        let result = classify_file_fast(&file);
        assert!(
            result.is_none(),
            "unknown extension with no strong signal should not fast-classify"
        );
    }

    #[test]
    fn fast_classify_archive_with_descriptive_parent() {
        let file = make_file(
            "project_backups/backup_jan.zip",
            "project_backups",
            "zip",
            "application/zip",
        );
        let result = classify_file_fast(&file);
        assert!(result.is_some());
        assert_eq!(result.unwrap().folder, "project_backups");
    }

    #[test]
    fn fast_classify_code_needs_high_confidence() {
        // Code files max at 0.70 (path keyword 0.45 + ext 0.25) with current scoring,
        // so they go to LLM for more nuanced classification. The Code arm in
        // classify_file_fast exists for future scoring improvements.
        let file = make_file(
            "my_project/src/main.rs",
            "my_project/src",
            "rs",
            "text/x-rust",
        );
        let result = classify_file_fast(&file);
        assert!(
            result.is_none(),
            "code files below 0.80 confidence should go to LLM"
        );
    }

    #[test]
    fn smart_folder_name_uses_last_path_segment() {
        let file = make_file(
            "deep/nested/screenshots/grab.png",
            "deep/nested/screenshots",
            "png",
            "image/png",
        );
        let det = classify_file_deterministic(&file);
        let name = smart_folder_name(&file, &det);
        assert_eq!(name, "screenshots");
    }

    #[test]
    fn is_generic_parent_catches_common_names() {
        assert!(is_generic_parent("Downloads"));
        assert!(is_generic_parent("desktop"));
        assert!(is_generic_parent("TMP"));
        assert!(is_generic_parent("New Folder"));
        assert!(is_generic_parent(""));
        assert!(!is_generic_parent("vacation_photos"));
        assert!(!is_generic_parent("project_backups"));
    }
}
