use rusqlite::Connection;

use crate::error::AppError;

const SCHEMA_V1: &str = "
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    extension TEXT,
    mime_type TEXT,
    size_bytes INTEGER,
    created_at TEXT,
    modified_at TEXT,
    accessed_at TEXT,
    hash_sha256 TEXT,
    is_directory BOOLEAN DEFAULT 0,
    parent_path TEXT,
    indexed_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_files_path ON files(path);
CREATE INDEX IF NOT EXISTS idx_files_parent ON files(parent_path);
CREATE INDEX IF NOT EXISTS idx_files_extension ON files(extension);
CREATE INDEX IF NOT EXISTS idx_files_hash ON files(hash_sha256);

CREATE TABLE IF NOT EXISTS undo_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    operation_id TEXT UNIQUE NOT NULL,
    operation_type TEXT NOT NULL,
    forward_command TEXT NOT NULL,
    inverse_command TEXT NOT NULL,
    affected_paths TEXT NOT NULL,
    metadata TEXT,
    executed_at TEXT DEFAULT CURRENT_TIMESTAMP,
    undone BOOLEAN DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_undo_time ON undo_log(executed_at DESC);

CREATE TABLE IF NOT EXISTS permission_scopes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    directory_path TEXT UNIQUE NOT NULL,
    allow_content_scan BOOLEAN DEFAULT 0,
    allow_modification BOOLEAN DEFAULT 0,
    allow_ocr BOOLEAN DEFAULT 1,
    allow_indexing BOOLEAN DEFAULT 1,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
";

pub fn run_migrations(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch(SCHEMA_V1)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_creates_tables() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"files".to_string()));
        assert!(tables.contains(&"undo_log".to_string()));
        assert!(tables.contains(&"permission_scopes".to_string()));
    }

    #[test]
    fn test_migration_enables_wal() {
        let dir = std::env::temp_dir().join("frogger_test_wal");
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        let conn = Connection::open(&db_path).unwrap();
        run_migrations(&conn).unwrap();

        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");

        drop(conn);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_migration_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap(); // should not error
    }
}
