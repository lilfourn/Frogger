use rusqlite::{params, Connection};

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
        .map(|m| serde_json::to_string(m))
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

    fn setup_db() -> Connection {
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
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
