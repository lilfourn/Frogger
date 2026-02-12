use rusqlite::{params, Connection};
use std::collections::BTreeMap;
use std::path::Path;

use crate::error::AppError;
use crate::models::file_entry::FileEntry;
use crate::models::operation::{OperationRecord, OperationType};
use crate::models::search::{FtsResult, OcrRecord, VecResult};

pub fn insert_file(conn: &Connection, entry: &FileEntry) -> Result<i64, AppError> {
    conn.execute(
        "INSERT OR REPLACE INTO files (path, name, extension, mime_type, size_bytes, created_at, modified_at, is_directory, parent_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            entry.path,
            entry.name,
            entry.extension,
            entry.mime_type,
            entry.size_bytes,
            entry.created_at,
            entry.modified_at,
            entry.is_directory,
            entry.parent_path,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

#[allow(dead_code)]
pub fn list_by_parent(conn: &Connection, parent_path: &str) -> Result<Vec<FileEntry>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT path, name, extension, mime_type, size_bytes, created_at, modified_at, is_directory, parent_path
         FROM files WHERE parent_path = ?1 ORDER BY is_directory DESC, name ASC",
    )?;

    let entries = stmt
        .query_map(params![parent_path], |row| {
            Ok(FileEntry {
                path: row.get(0)?,
                name: row.get(1)?,
                extension: row.get(2)?,
                mime_type: row.get(3)?,
                size_bytes: row.get(4)?,
                created_at: row.get(5)?,
                modified_at: row.get(6)?,
                is_directory: row.get(7)?,
                parent_path: row.get(8)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(entries)
}

#[allow(dead_code)]
pub fn get_by_path(conn: &Connection, path: &str) -> Result<Option<FileEntry>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT path, name, extension, mime_type, size_bytes, created_at, modified_at, is_directory, parent_path
         FROM files WHERE path = ?1",
    )?;

    let entry = stmt
        .query_row(params![path], |row| {
            Ok(FileEntry {
                path: row.get(0)?,
                name: row.get(1)?,
                extension: row.get(2)?,
                mime_type: row.get(3)?,
                size_bytes: row.get(4)?,
                created_at: row.get(5)?,
                modified_at: row.get(6)?,
                is_directory: row.get(7)?,
                parent_path: row.get(8)?,
            })
        })
        .optional()?;

    Ok(entry)
}

#[allow(dead_code)]
pub fn delete_by_path(conn: &Connection, path: &str) -> Result<usize, AppError> {
    let count = conn.execute("DELETE FROM files WHERE path = ?1", params![path])?;
    Ok(count)
}

pub fn insert_operation(conn: &Connection, record: &OperationRecord) -> Result<i64, AppError> {
    let affected_json = serde_json::to_string(&record.affected_paths)?;
    let metadata_json = record
        .metadata
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;

    conn.execute(
        "INSERT INTO undo_log (operation_id, operation_type, forward_command, inverse_command, affected_paths, metadata, executed_at, undone)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            record.operation_id,
            record.operation_type.to_string(),
            record.forward_command,
            record.inverse_command,
            affected_json,
            metadata_json,
            record.executed_at,
            record.undone,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_latest_undoable(conn: &Connection) -> Result<Option<OperationRecord>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT operation_id, operation_type, forward_command, inverse_command, affected_paths, metadata, executed_at, undone
         FROM undo_log WHERE undone = 0 ORDER BY executed_at DESC LIMIT 1",
    )?;

    let record = stmt
        .query_row([], |row| {
            let affected_str: String = row.get(4)?;
            let metadata_str: Option<String> = row.get(5)?;
            let op_type_str: String = row.get(1)?;

            Ok(OperationRecord {
                operation_id: row.get(0)?,
                operation_type: op_type_str
                    .parse::<OperationType>()
                    .unwrap_or(OperationType::Move),
                forward_command: row.get(2)?,
                inverse_command: row.get(3)?,
                affected_paths: serde_json::from_str(&affected_str).unwrap_or_default(),
                metadata: metadata_str.and_then(|s| serde_json::from_str(&s).ok()),
                executed_at: row.get(6)?,
                undone: row.get(7)?,
            })
        })
        .optional()?;

    Ok(record)
}

pub fn mark_undone(conn: &Connection, operation_id: &str) -> Result<usize, AppError> {
    let count = conn.execute(
        "UPDATE undo_log SET undone = 1 WHERE operation_id = ?1",
        params![operation_id],
    )?;
    Ok(count)
}

pub fn get_latest_redoable(conn: &Connection) -> Result<Option<OperationRecord>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT operation_id, operation_type, forward_command, inverse_command, affected_paths, metadata, executed_at, undone
         FROM undo_log WHERE undone = 1 ORDER BY executed_at DESC LIMIT 1",
    )?;

    let record = stmt
        .query_row([], |row| {
            let affected_str: String = row.get(4)?;
            let metadata_str: Option<String> = row.get(5)?;
            let op_type_str: String = row.get(1)?;

            Ok(OperationRecord {
                operation_id: row.get(0)?,
                operation_type: op_type_str
                    .parse::<OperationType>()
                    .unwrap_or(OperationType::Move),
                forward_command: row.get(2)?,
                inverse_command: row.get(3)?,
                affected_paths: serde_json::from_str(&affected_str).unwrap_or_default(),
                metadata: metadata_str.and_then(|s| serde_json::from_str(&s).ok()),
                executed_at: row.get(6)?,
                undone: row.get(7)?,
            })
        })
        .optional()?;

    Ok(record)
}

pub fn mark_not_undone(conn: &Connection, operation_id: &str) -> Result<usize, AppError> {
    let count = conn.execute(
        "UPDATE undo_log SET undone = 0 WHERE operation_id = ?1",
        params![operation_id],
    )?;
    Ok(count)
}

// --- OCR text ---

pub fn insert_ocr_text(
    conn: &Connection,
    file_path: &str,
    text_content: &str,
    language: &str,
    confidence: Option<f64>,
    processed_at: &str,
) -> Result<i64, AppError> {
    conn.execute(
        "INSERT OR REPLACE INTO ocr_text (file_path, text_content, language, confidence, processed_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![file_path, text_content, language, confidence, processed_at],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_ocr_text(conn: &Connection, file_path: &str) -> Result<Option<OcrRecord>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT file_path, text_content, language, confidence, processed_at
         FROM ocr_text WHERE file_path = ?1",
    )?;
    let record = stmt
        .query_row(params![file_path], |row| {
            Ok(OcrRecord {
                file_path: row.get(0)?,
                text_content: row.get(1)?,
                language: row.get(2)?,
                confidence: row.get(3)?,
                processed_at: row.get(4)?,
            })
        })
        .optional()?;
    Ok(record)
}

#[allow(dead_code)]
pub fn delete_ocr_text(conn: &Connection, file_path: &str) -> Result<usize, AppError> {
    let count = conn.execute(
        "DELETE FROM ocr_text WHERE file_path = ?1",
        params![file_path],
    )?;
    Ok(count)
}

// --- FTS5 ---

pub fn insert_fts(
    conn: &Connection,
    file_path: &str,
    file_name: &str,
    ocr_text: &str,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT OR REPLACE INTO files_fts(file_path, file_name, ocr_text) VALUES (?1, ?2, ?3)",
        params![file_path, file_name, ocr_text],
    )?;
    Ok(())
}

#[allow(dead_code)]
pub fn search_fts(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<FtsResult>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT file_path FROM files_fts WHERE files_fts MATCH ?1 ORDER BY bm25(files_fts, 2.0, 10.0, 1.0) LIMIT ?2",
    )?;
    let results = stmt
        .query_map(params![query, limit as i64], |row| {
            Ok(FtsResult {
                file_path: row.get(0)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(results)
}

fn escape_like_pattern(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        if matches!(ch, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

pub fn search_file_paths_by_name_or_path(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<FtsResult>, AppError> {
    let trimmed = query.trim();
    if trimmed.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }

    let escaped = escape_like_pattern(trimmed);
    let prefix = format!("{escaped}%");
    let contains = format!("%{escaped}%");

    let mut stmt = conn.prepare(
        "SELECT path
         FROM files
         WHERE name = ?1 COLLATE NOCASE
            OR path = ?1 COLLATE NOCASE
            OR name LIKE ?2 ESCAPE '\\' COLLATE NOCASE
            OR path LIKE ?2 ESCAPE '\\' COLLATE NOCASE
            OR name LIKE ?3 ESCAPE '\\' COLLATE NOCASE
            OR path LIKE ?3 ESCAPE '\\' COLLATE NOCASE
         ORDER BY
            CASE
              WHEN name = ?1 COLLATE NOCASE THEN 0
              WHEN path = ?1 COLLATE NOCASE THEN 1
              WHEN name LIKE ?2 ESCAPE '\\' COLLATE NOCASE THEN 2
              WHEN path LIKE ?2 ESCAPE '\\' COLLATE NOCASE THEN 3
              WHEN name LIKE ?3 ESCAPE '\\' COLLATE NOCASE THEN 4
              WHEN path LIKE ?3 ESCAPE '\\' COLLATE NOCASE THEN 5
              ELSE 6
            END,
            is_directory DESC,
            length(path) ASC,
            name COLLATE NOCASE ASC
         LIMIT ?4",
    )?;

    let results = stmt
        .query_map(params![trimmed, prefix, contains, limit as i64], |row| {
            Ok(FtsResult {
                file_path: row.get(0)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(results)
}

// --- Vector index ---

pub fn f32_to_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub fn insert_vec(conn: &Connection, file_path: &str, embedding: &[f32]) -> Result<(), AppError> {
    let bytes = f32_to_bytes(embedding);
    conn.execute(
        "INSERT OR REPLACE INTO vec_index(file_path, embedding) VALUES (?1, ?2)",
        params![file_path, bytes],
    )?;
    Ok(())
}

pub fn search_vec(
    conn: &Connection,
    query_embedding: &[f32],
    limit: usize,
) -> Result<Vec<VecResult>, AppError> {
    let bytes = f32_to_bytes(query_embedding);
    let mut stmt = conn
        .prepare("SELECT file_path, distance FROM vec_index WHERE embedding MATCH ?1 AND k = ?2")?;
    let results = stmt
        .query_map(params![bytes, limit as i64], |row| {
            Ok(VecResult {
                file_path: row.get(0)?,
                distance: row.get(1)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(results)
}

// --- Incremental indexing ---

pub fn needs_reindex(conn: &Connection, file_path: &str, modified_at: &str) -> bool {
    match conn
        .query_row(
            "SELECT modified_at FROM files WHERE path = ?1",
            params![file_path],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
    {
        Ok(Some(Some(db_modified))) => db_modified.as_str() < modified_at,
        _ => true,
    }
}

// --- Chat history ---

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct ChatRecord {
    pub id: i64,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

pub fn insert_chat_message(
    conn: &Connection,
    session_id: &str,
    role: &str,
    content: &str,
) -> Result<i64, AppError> {
    conn.execute(
        "INSERT INTO chat_history (session_id, role, content) VALUES (?1, ?2, ?3)",
        params![session_id, role, content],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_chat_messages(conn: &Connection, session_id: &str) -> Result<Vec<ChatRecord>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, role, content, created_at FROM chat_history WHERE session_id = ?1 ORDER BY id ASC",
    )?;
    let records = stmt
        .query_map(params![session_id], |row| {
            Ok(ChatRecord {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(records)
}

pub fn delete_chat_session(conn: &Connection, session_id: &str) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM chat_history WHERE session_id = ?1",
        params![session_id],
    )?;
    Ok(())
}

// --- Permission scopes ---

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct PermissionScope {
    pub id: i64,
    pub directory_path: String,
    pub content_scan_mode: String,
    pub modification_mode: String,
    pub ocr_mode: String,
    pub indexing_mode: String,
    pub created_at: String,
}

pub fn get_permission_scopes(conn: &Connection) -> Result<Vec<PermissionScope>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, directory_path, content_scan_mode, modification_mode, ocr_mode, indexing_mode, created_at
         FROM permission_scopes ORDER BY length(directory_path) DESC, directory_path ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(PermissionScope {
                id: row.get(0)?,
                directory_path: row.get(1)?,
                content_scan_mode: row.get(2)?,
                modification_mode: row.get(3)?,
                ocr_mode: row.get(4)?,
                indexing_mode: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

pub fn upsert_permission_scope(
    conn: &Connection,
    directory_path: &str,
    content_scan_mode: &str,
    modification_mode: &str,
    ocr_mode: &str,
    indexing_mode: &str,
) -> Result<i64, AppError> {
    let allow_content_scan = content_scan_mode == "allow";
    let allow_modification = modification_mode == "allow";
    let allow_ocr = ocr_mode == "allow";
    let allow_indexing = indexing_mode == "allow";

    conn.execute(
        "INSERT INTO permission_scopes (
            directory_path,
            allow_content_scan, allow_modification, allow_ocr, allow_indexing,
            content_scan_mode, modification_mode, ocr_mode, indexing_mode
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(directory_path) DO UPDATE SET
           allow_content_scan = excluded.allow_content_scan,
           allow_modification = excluded.allow_modification,
           allow_ocr = excluded.allow_ocr,
           allow_indexing = excluded.allow_indexing,
           content_scan_mode = excluded.content_scan_mode,
           modification_mode = excluded.modification_mode,
           ocr_mode = excluded.ocr_mode,
           indexing_mode = excluded.indexing_mode",
        params![
            directory_path,
            allow_content_scan,
            allow_modification,
            allow_ocr,
            allow_indexing,
            content_scan_mode,
            modification_mode,
            ocr_mode,
            indexing_mode,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_permission_scope(conn: &Connection, id: i64) -> Result<usize, AppError> {
    let count = conn.execute("DELETE FROM permission_scopes WHERE id = ?1", params![id])?;
    Ok(count)
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub struct PermissionScopeNormalizationReport {
    pub scanned: usize,
    pub normalized: usize,
    pub merged: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone)]
struct PermissionScopeModes {
    content_scan_mode: String,
    modification_mode: String,
    ocr_mode: String,
    indexing_mode: String,
}

impl PermissionScopeModes {
    fn from_scope(scope: &PermissionScope) -> Self {
        Self {
            content_scan_mode: scope.content_scan_mode.clone(),
            modification_mode: scope.modification_mode.clone(),
            ocr_mode: scope.ocr_mode.clone(),
            indexing_mode: scope.indexing_mode.clone(),
        }
    }

    fn merge_restrictive(&mut self, other: &Self) {
        self.content_scan_mode =
            most_restrictive_mode(&self.content_scan_mode, &other.content_scan_mode).to_string();
        self.modification_mode =
            most_restrictive_mode(&self.modification_mode, &other.modification_mode).to_string();
        self.ocr_mode = most_restrictive_mode(&self.ocr_mode, &other.ocr_mode).to_string();
        self.indexing_mode =
            most_restrictive_mode(&self.indexing_mode, &other.indexing_mode).to_string();
    }
}

fn permission_mode_rank(mode: &str) -> u8 {
    match mode {
        "deny" => 3,
        "ask" => 2,
        "allow" => 1,
        _ => 0,
    }
}

fn most_restrictive_mode<'a>(left: &'a str, right: &'a str) -> &'a str {
    if permission_mode_rank(left) >= permission_mode_rank(right) {
        left
    } else {
        right
    }
}

fn normalize_permission_scope_path(path: &str) -> String {
    let mut normalized = path.trim().replace('\\', "/");
    while normalized.ends_with('/') && normalized.len() > 1 {
        normalized.pop();
    }
    normalized
}

fn is_windows_drive_root(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() == 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

fn parent_permission_scope_path(path: &str) -> Option<String> {
    let normalized = normalize_permission_scope_path(path);
    if normalized == "/" || is_windows_drive_root(&normalized) {
        return None;
    }
    let index = normalized.rfind('/')?;
    if index == 0 {
        return Some("/".to_string());
    }
    Some(normalized[..index].to_string())
}

fn looks_like_file_name(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.starts_with('.')
        || name.ends_with('.')
    {
        return false;
    }
    let Some((_, extension)) = name.rsplit_once('.') else {
        return false;
    };
    !extension.is_empty()
        && extension.len() <= 12
        && extension.chars().all(|ch| ch.is_ascii_alphanumeric())
}

fn infer_directory_scope_path(raw_path: &str) -> (String, bool) {
    let normalized = normalize_permission_scope_path(raw_path);
    if normalized.is_empty() {
        return (normalized, true);
    }

    if let Ok(metadata) = std::fs::metadata(Path::new(raw_path)) {
        if metadata.is_file() {
            if let Some(parent) = parent_permission_scope_path(&normalized) {
                return (parent, false);
            }
            return (normalized, true);
        }
        if metadata.is_dir() {
            return (normalized, false);
        }
    }

    if looks_like_file_name(&normalized) {
        if let Some(parent) = parent_permission_scope_path(&normalized) {
            return (parent, false);
        }
        return (normalized, true);
    }

    (normalized, false)
}

pub fn normalize_permission_scopes(
    conn: &Connection,
) -> Result<PermissionScopeNormalizationReport, AppError> {
    conn.execute_batch("BEGIN IMMEDIATE TRANSACTION")?;

    let result: Result<PermissionScopeNormalizationReport, AppError> = (|| {
        let scopes = get_permission_scopes(conn)?;
        let mut report = PermissionScopeNormalizationReport {
            scanned: scopes.len(),
            ..PermissionScopeNormalizationReport::default()
        };
        if scopes.is_empty() {
            return Ok(report);
        }

        let mut merged_by_path: BTreeMap<String, PermissionScopeModes> = BTreeMap::new();
        let mut ids_to_delete: Vec<i64> = Vec::new();

        for scope in &scopes {
            let source = normalize_permission_scope_path(&scope.directory_path);
            let (target, skipped) = infer_directory_scope_path(&scope.directory_path);
            if skipped {
                report.skipped += 1;
            }
            if target.is_empty() {
                continue;
            }
            if target != source {
                report.normalized += 1;
            }
            if scope.directory_path != target {
                ids_to_delete.push(scope.id);
            }

            let incoming_modes = PermissionScopeModes::from_scope(scope);
            if let Some(existing_modes) = merged_by_path.get_mut(&target) {
                report.merged += 1;
                existing_modes.merge_restrictive(&incoming_modes);
            } else {
                merged_by_path.insert(target, incoming_modes);
            }
        }

        for (directory_path, modes) in merged_by_path {
            upsert_permission_scope(
                conn,
                &directory_path,
                &modes.content_scan_mode,
                &modes.modification_mode,
                &modes.ocr_mode,
                &modes.indexing_mode,
            )?;
        }

        for scope_id in ids_to_delete {
            delete_permission_scope(conn, scope_id)?;
        }

        Ok(report)
    })();

    match result {
        Ok(report) => {
            conn.execute_batch("COMMIT")?;
            Ok(report)
        }
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

#[allow(dead_code)]
pub fn check_permission(conn: &Connection, path: &str, field: &str) -> Result<bool, AppError> {
    let mode_col = match field {
        "content_scan" => "content_scan_mode",
        "modification" => "modification_mode",
        "ocr" => "ocr_mode",
        "indexing" => "indexing_mode",
        _ => return Ok(true),
    };

    fn normalize(p: &str) -> String {
        let mut out = p.replace('\\', "/");
        while out.ends_with('/') && out.len() > 1 {
            out.pop();
        }
        out
    }

    fn matches(path: &str, scope: &str) -> bool {
        let path = normalize(path);
        let scope = normalize(scope);
        if scope == "/" {
            return path.starts_with('/');
        }
        path == scope || path.starts_with(&(scope + "/"))
    }

    let query = format!(
        "SELECT directory_path, {mode_col}
         FROM permission_scopes
         ORDER BY length(directory_path) DESC, directory_path ASC"
    );
    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query_map([], |row| {
        let scope_path: String = row.get(0)?;
        let mode: String = row.get(1)?;
        Ok((scope_path, mode))
    })?;
    for row in rows {
        let (scope_path, mode) = row?;
        if matches(path, &scope_path) {
            return Ok(mode == "allow");
        }
    }
    Ok(true)
}

// --- Audit log ---

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct AuditLogEntry {
    pub id: i64,
    pub endpoint: String,
    pub request_summary: Option<String>,
    pub tokens_used: Option<i64>,
    pub cost_usd: Option<f64>,
    pub created_at: String,
}

pub fn insert_audit_log(
    conn: &Connection,
    endpoint: &str,
    request_summary: Option<&str>,
    tokens_used: Option<i64>,
    cost_usd: Option<f64>,
) -> Result<i64, AppError> {
    conn.execute(
        "INSERT INTO api_audit_log (endpoint, request_summary, tokens_used, cost_usd) VALUES (?1, ?2, ?3, ?4)",
        params![endpoint, request_summary, tokens_used, cost_usd],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_audit_log(conn: &Connection, limit: usize) -> Result<Vec<AuditLogEntry>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, endpoint, request_summary, tokens_used, cost_usd, created_at
         FROM api_audit_log ORDER BY created_at DESC LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit as i64], |row| {
            Ok(AuditLogEntry {
                id: row.get(0)?,
                endpoint: row.get(1)?,
                request_summary: row.get(2)?,
                tokens_used: row.get(3)?,
                cost_usd: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

// --- Settings ---

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), AppError> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, AppError> {
    let result = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()?;
    Ok(result)
}

#[allow(dead_code)]
pub fn delete_setting(conn: &Connection, key: &str) -> Result<(), AppError> {
    conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
    Ok(())
}

// --- Cascade delete (removes file from all index tables) ---

pub fn delete_file_index(conn: &Connection, file_path: &str) -> Result<(), AppError> {
    conn.execute("DELETE FROM files WHERE path = ?1", params![file_path])?;
    conn.execute(
        "DELETE FROM ocr_text WHERE file_path = ?1",
        params![file_path],
    )?;
    conn.execute(
        "DELETE FROM files_fts WHERE file_path = ?1",
        params![file_path],
    )?;
    conn.execute(
        "DELETE FROM vec_index WHERE file_path = ?1",
        params![file_path],
    )?;
    Ok(())
}

// Needed for rusqlite optional query results
trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::migrations::run_migrations;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn setup_db() -> Connection {
        crate::data::register_sqlite_vec_extension();
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        conn
    }

    fn sample_file() -> FileEntry {
        FileEntry {
            path: "/home/user/docs/readme.md".to_string(),
            name: "readme.md".to_string(),
            extension: Some("md".to_string()),
            mime_type: Some("text/markdown".to_string()),
            size_bytes: Some(1024),
            created_at: Some("2025-01-01T00:00:00Z".to_string()),
            modified_at: Some("2025-01-02T00:00:00Z".to_string()),
            is_directory: false,
            parent_path: Some("/home/user/docs".to_string()),
        }
    }

    fn temp_permission_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "frogger-permission-normalize-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn path_str(path: &Path) -> String {
        path.to_string_lossy().to_string()
    }

    #[test]
    fn test_file_crud() {
        let conn = setup_db();
        let file = sample_file();

        // Insert
        let id = insert_file(&conn, &file).unwrap();
        assert!(id > 0);

        // Get by path
        let fetched = get_by_path(&conn, &file.path).unwrap().unwrap();
        assert_eq!(fetched.name, "readme.md");
        assert_eq!(fetched.size_bytes, Some(1024));

        // List by parent
        let list = list_by_parent(&conn, "/home/user/docs").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "readme.md");

        // Delete
        let count = delete_by_path(&conn, &file.path).unwrap();
        assert_eq!(count, 1);

        let gone = get_by_path(&conn, &file.path).unwrap();
        assert!(gone.is_none());
    }

    #[test]
    fn test_list_by_parent_sorts_dirs_first() {
        let conn = setup_db();

        let dir = FileEntry {
            path: "/home/user/docs/subdir".to_string(),
            name: "subdir".to_string(),
            extension: None,
            mime_type: None,
            size_bytes: None,
            created_at: None,
            modified_at: None,
            is_directory: true,
            parent_path: Some("/home/user/docs".to_string()),
        };
        let file = sample_file();

        insert_file(&conn, &file).unwrap();
        insert_file(&conn, &dir).unwrap();

        let list = list_by_parent(&conn, "/home/user/docs").unwrap();
        assert_eq!(list.len(), 2);
        assert!(list[0].is_directory); // dir first
        assert!(!list[1].is_directory); // file second
    }

    #[test]
    fn test_insert_file_upsert() {
        let conn = setup_db();
        let mut file = sample_file();

        insert_file(&conn, &file).unwrap();
        file.size_bytes = Some(2048);
        insert_file(&conn, &file).unwrap(); // should upsert

        let fetched = get_by_path(&conn, &file.path).unwrap().unwrap();
        assert_eq!(fetched.size_bytes, Some(2048));
    }

    #[test]
    fn test_operation_crud() {
        let conn = setup_db();

        let record = OperationRecord {
            operation_id: uuid::Uuid::new_v4().to_string(),
            operation_type: OperationType::Rename,
            forward_command: "mv old.txt new.txt".to_string(),
            inverse_command: "mv new.txt old.txt".to_string(),
            affected_paths: vec!["/home/user/old.txt".to_string()],
            metadata: None,
            executed_at: chrono::Utc::now().to_rfc3339(),
            undone: false,
        };

        let id = insert_operation(&conn, &record).unwrap();
        assert!(id > 0);

        // Get latest undoable
        let latest = get_latest_undoable(&conn).unwrap().unwrap();
        assert_eq!(latest.operation_id, record.operation_id);
        assert!(!latest.undone);

        // Mark undone
        mark_undone(&conn, &record.operation_id).unwrap();
        let undoable = get_latest_undoable(&conn).unwrap();
        assert!(undoable.is_none()); // nothing left to undo

        // Get latest redoable
        let redoable = get_latest_redoable(&conn).unwrap().unwrap();
        assert_eq!(redoable.operation_id, record.operation_id);

        // Mark not undone (redo)
        mark_not_undone(&conn, &record.operation_id).unwrap();
        let back = get_latest_undoable(&conn).unwrap().unwrap();
        assert_eq!(back.operation_id, record.operation_id);
    }

    #[test]
    fn test_ocr_text_crud() {
        let conn = setup_db();
        let path = "/home/user/image.png";

        let id = insert_ocr_text(
            &conn,
            path,
            "Hello world",
            "eng",
            Some(0.95),
            "2025-01-01T00:00:00Z",
        )
        .unwrap();
        assert!(id > 0);

        let record = get_ocr_text(&conn, path).unwrap().unwrap();
        assert_eq!(record.text_content, "Hello world");
        assert_eq!(record.language, "eng");
        assert!((record.confidence.unwrap() - 0.95).abs() < f64::EPSILON);

        let count = delete_ocr_text(&conn, path).unwrap();
        assert_eq!(count, 1);
        assert!(get_ocr_text(&conn, path).unwrap().is_none());
    }

    #[test]
    fn test_fts_insert_and_search() {
        let conn = setup_db();

        insert_fts(
            &conn,
            "/home/user/readme.md",
            "readme.md",
            "setup instructions",
        )
        .unwrap();
        insert_fts(&conn, "/home/user/notes.txt", "notes.txt", "shopping list").unwrap();

        let results = search_fts(&conn, "readme", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "/home/user/readme.md");

        let results = search_fts(&conn, "instructions", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "/home/user/readme.md");
    }

    #[test]
    fn test_fts_filename_ranked_above_content() {
        let conn = setup_db();

        insert_fts(
            &conn,
            "/docs/report.txt",
            "report.txt",
            "contains frogger reference",
        )
        .unwrap();
        insert_fts(&conn, "/projects/frogger.md", "frogger.md", "project notes").unwrap();

        let results = search_fts(&conn, "frogger", 10).unwrap();
        assert!(results.len() >= 2);
        assert_eq!(results[0].file_path, "/projects/frogger.md");
    }

    #[test]
    fn test_search_file_paths_by_name_or_path_prioritizes_exact_and_directories() {
        let conn = setup_db();

        let exact_dir = FileEntry {
            path: "/home/user/Reports".to_string(),
            name: "Reports".to_string(),
            extension: None,
            mime_type: None,
            size_bytes: None,
            created_at: None,
            modified_at: None,
            is_directory: true,
            parent_path: Some("/home/user".to_string()),
        };
        let prefix_file = FileEntry {
            path: "/home/user/reports.txt".to_string(),
            name: "reports.txt".to_string(),
            extension: Some("txt".to_string()),
            mime_type: Some("text/plain".to_string()),
            size_bytes: Some(123),
            created_at: None,
            modified_at: None,
            is_directory: false,
            parent_path: Some("/home/user".to_string()),
        };
        let contains_path_file = FileEntry {
            path: "/home/user/archive/old_reports_note.txt".to_string(),
            name: "note.txt".to_string(),
            extension: Some("txt".to_string()),
            mime_type: Some("text/plain".to_string()),
            size_bytes: Some(321),
            created_at: None,
            modified_at: None,
            is_directory: false,
            parent_path: Some("/home/user/archive".to_string()),
        };

        insert_file(&conn, &prefix_file).unwrap();
        insert_file(&conn, &contains_path_file).unwrap();
        insert_file(&conn, &exact_dir).unwrap();

        let results = search_file_paths_by_name_or_path(&conn, "reports", 10).unwrap();
        assert!(results.len() >= 3);
        assert_eq!(results[0].file_path, "/home/user/Reports");
        assert_eq!(results[1].file_path, "/home/user/reports.txt");
        assert!(results
            .iter()
            .any(|result| result.file_path == "/home/user/archive/old_reports_note.txt"));
    }

    #[test]
    fn test_vec_insert_and_search() {
        let conn = setup_db();

        let mut emb1 = vec![0.0f32; 384];
        emb1[0] = 1.0;
        let mut emb2 = vec![0.0f32; 384];
        emb2[1] = 1.0;

        insert_vec(&conn, "/file_a.txt", &emb1).unwrap();
        insert_vec(&conn, "/file_b.txt", &emb2).unwrap();

        let results = search_vec(&conn, &emb1, 2).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].file_path, "/file_a.txt");
        assert!(results[0].distance < results[1].distance);
    }

    #[test]
    fn test_delete_file_index_cascades() {
        let conn = setup_db();
        let path = "/home/user/doc.pdf";

        let file = FileEntry {
            path: path.to_string(),
            name: "doc.pdf".to_string(),
            extension: Some("pdf".to_string()),
            mime_type: Some("application/pdf".to_string()),
            size_bytes: Some(5000),
            created_at: None,
            modified_at: None,
            is_directory: false,
            parent_path: Some("/home/user".to_string()),
        };
        insert_file(&conn, &file).unwrap();
        insert_ocr_text(
            &conn,
            path,
            "tax return",
            "eng",
            None,
            "2025-01-01T00:00:00Z",
        )
        .unwrap();
        insert_fts(&conn, path, "doc.pdf", "tax return").unwrap();
        insert_vec(&conn, path, &vec![0.1f32; 384]).unwrap();

        delete_file_index(&conn, path).unwrap();

        assert!(get_by_path(&conn, path).unwrap().is_none());
        assert!(get_ocr_text(&conn, path).unwrap().is_none());
        assert!(search_fts(&conn, "tax", 10).unwrap().is_empty());
        assert!(search_vec(&conn, &vec![0.1f32; 384], 10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_chat_message_crud() {
        let conn = setup_db();
        let session = "test-session-1";

        let msgs = get_chat_messages(&conn, session).unwrap();
        assert!(msgs.is_empty());

        insert_chat_message(&conn, session, "user", "Hello").unwrap();
        insert_chat_message(&conn, session, "assistant", "Hi there").unwrap();

        let msgs = get_chat_messages(&conn, session).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[0].content, "Hello");
        assert_eq!(msgs[1].role, "assistant");
        assert_eq!(msgs[1].content, "Hi there");

        // different session is isolated
        insert_chat_message(&conn, "other-session", "user", "Different").unwrap();
        assert_eq!(get_chat_messages(&conn, session).unwrap().len(), 2);

        delete_chat_session(&conn, session).unwrap();
        assert!(get_chat_messages(&conn, session).unwrap().is_empty());
        assert_eq!(get_chat_messages(&conn, "other-session").unwrap().len(), 1);
    }

    #[test]
    fn test_settings_crud() {
        let conn = setup_db();

        assert!(get_setting(&conn, "api_key").unwrap().is_none());

        set_setting(&conn, "api_key", "sk-test-123").unwrap();
        assert_eq!(
            get_setting(&conn, "api_key").unwrap().unwrap(),
            "sk-test-123"
        );

        set_setting(&conn, "api_key", "sk-updated").unwrap();
        assert_eq!(
            get_setting(&conn, "api_key").unwrap().unwrap(),
            "sk-updated"
        );

        delete_setting(&conn, "api_key").unwrap();
        assert!(get_setting(&conn, "api_key").unwrap().is_none());
    }

    #[test]
    fn test_normalize_permission_scope_file_path_to_parent() {
        let conn = setup_db();
        let temp_dir = temp_permission_dir();
        let file_path = temp_dir.join("invoice.pdf");
        fs::write(&file_path, b"stub").unwrap();

        upsert_permission_scope(
            &conn,
            &path_str(&file_path),
            "allow",
            "ask",
            "allow",
            "allow",
        )
        .unwrap();

        let report = normalize_permission_scopes(&conn).unwrap();
        assert_eq!(report.scanned, 1);
        assert_eq!(report.normalized, 1);

        let scopes = get_permission_scopes(&conn).unwrap();
        assert_eq!(scopes.len(), 1);
        assert_eq!(scopes[0].directory_path, path_str(temp_dir.as_path()));

        let _ = fs::remove_file(file_path);
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_normalize_permission_scopes_merges_to_most_restrictive_modes() {
        let conn = setup_db();
        let temp_dir = temp_permission_dir();
        let first_file = temp_dir.join("a.txt");
        let second_file = temp_dir.join("b.txt");
        fs::write(&first_file, b"a").unwrap();
        fs::write(&second_file, b"b").unwrap();

        upsert_permission_scope(
            &conn,
            &path_str(&first_file),
            "allow",
            "allow",
            "allow",
            "allow",
        )
        .unwrap();
        upsert_permission_scope(
            &conn,
            &path_str(&second_file),
            "deny",
            "ask",
            "allow",
            "deny",
        )
        .unwrap();

        let report = normalize_permission_scopes(&conn).unwrap();
        assert_eq!(report.normalized, 2);
        assert_eq!(report.merged, 1);

        let scopes = get_permission_scopes(&conn).unwrap();
        assert_eq!(scopes.len(), 1);
        assert_eq!(scopes[0].directory_path, path_str(temp_dir.as_path()));
        assert_eq!(scopes[0].content_scan_mode, "deny");
        assert_eq!(scopes[0].modification_mode, "ask");
        assert_eq!(scopes[0].ocr_mode, "allow");
        assert_eq!(scopes[0].indexing_mode, "deny");

        let _ = fs::remove_file(first_file);
        let _ = fs::remove_file(second_file);
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_normalize_permission_scope_keeps_ambiguous_hidden_path() {
        let conn = setup_db();
        let hidden_like_path = "/Users/test/.config";
        upsert_permission_scope(&conn, hidden_like_path, "ask", "allow", "allow", "allow").unwrap();

        let report = normalize_permission_scopes(&conn).unwrap();
        assert_eq!(report.normalized, 0);

        let scopes = get_permission_scopes(&conn).unwrap();
        assert_eq!(scopes.len(), 1);
        assert_eq!(scopes[0].directory_path, hidden_like_path);
    }

    #[test]
    fn test_normalize_permission_scopes_is_idempotent() {
        let conn = setup_db();
        let temp_dir = temp_permission_dir();
        let file_path = temp_dir.join("notes.txt");
        fs::write(&file_path, b"notes").unwrap();

        upsert_permission_scope(&conn, &path_str(&file_path), "ask", "ask", "allow", "allow")
            .unwrap();

        let first = normalize_permission_scopes(&conn).unwrap();
        let second = normalize_permission_scopes(&conn).unwrap();

        assert_eq!(first.normalized, 1);
        assert_eq!(second.normalized, 0);
        assert_eq!(second.merged, 0);
        assert_eq!(get_permission_scopes(&conn).unwrap().len(), 1);

        let _ = fs::remove_file(file_path);
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_needs_reindex() {
        let conn = setup_db();
        let path = "/home/user/test.txt";

        assert!(needs_reindex(&conn, path, "2025-06-01T00:00:00Z"));

        let file = FileEntry {
            path: path.to_string(),
            name: "test.txt".to_string(),
            extension: Some("txt".to_string()),
            mime_type: None,
            size_bytes: Some(100),
            created_at: None,
            modified_at: Some("2025-06-01T00:00:00Z".to_string()),
            is_directory: false,
            parent_path: Some("/home/user".to_string()),
        };
        insert_file(&conn, &file).unwrap();

        assert!(!needs_reindex(&conn, path, "2025-06-01T00:00:00Z"));
        assert!(!needs_reindex(&conn, path, "2025-05-01T00:00:00Z"));
        assert!(needs_reindex(&conn, path, "2025-07-01T00:00:00Z"));
    }
}
