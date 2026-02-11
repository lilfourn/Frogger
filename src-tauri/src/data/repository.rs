use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::models::file_entry::FileEntry;
use crate::models::operation::{OperationRecord, OperationType};

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
}
