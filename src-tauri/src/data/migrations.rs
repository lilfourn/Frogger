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
    content_scan_mode TEXT NOT NULL DEFAULT 'ask',
    modification_mode TEXT NOT NULL DEFAULT 'ask',
    ocr_mode TEXT NOT NULL DEFAULT 'allow',
    indexing_mode TEXT NOT NULL DEFAULT 'allow',
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
";

const SCHEMA_V2: &str = "
CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
    file_path,
    file_name,
    ocr_text,
    tokenize='porter unicode61'
);

CREATE TABLE IF NOT EXISTS ocr_text (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT UNIQUE NOT NULL,
    text_content TEXT NOT NULL,
    language TEXT DEFAULT 'eng',
    confidence REAL,
    processed_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ocr_file_path ON ocr_text(file_path);

CREATE TABLE IF NOT EXISTS ai_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT UNIQUE NOT NULL,
    description TEXT,
    tags TEXT,
    generated_at TEXT NOT NULL,
    model_version TEXT
);

CREATE TABLE IF NOT EXISTS chat_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS api_audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    endpoint TEXT NOT NULL,
    request_summary TEXT,
    tokens_used INTEGER,
    cost_usd REAL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
";

const SCHEMA_V3: &str = "
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
";

const SCHEMA_V2_VEC: &str =
    "CREATE VIRTUAL TABLE IF NOT EXISTS vec_index USING vec0(file_path TEXT PRIMARY KEY, embedding float[384])";

fn ensure_permission_mode_columns(conn: &Connection) -> Result<(), AppError> {
    let columns: Vec<String> = conn
        .prepare("PRAGMA table_info(permission_scopes)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut added = false;

    if !columns.iter().any(|c| c == "content_scan_mode") {
        conn.execute(
            "ALTER TABLE permission_scopes ADD COLUMN content_scan_mode TEXT NOT NULL DEFAULT 'ask'",
            [],
        )?;
        added = true;
    }
    if !columns.iter().any(|c| c == "modification_mode") {
        conn.execute(
            "ALTER TABLE permission_scopes ADD COLUMN modification_mode TEXT NOT NULL DEFAULT 'ask'",
            [],
        )?;
        added = true;
    }
    if !columns.iter().any(|c| c == "ocr_mode") {
        conn.execute(
            "ALTER TABLE permission_scopes ADD COLUMN ocr_mode TEXT NOT NULL DEFAULT 'allow'",
            [],
        )?;
        added = true;
    }
    if !columns.iter().any(|c| c == "indexing_mode") {
        conn.execute(
            "ALTER TABLE permission_scopes ADD COLUMN indexing_mode TEXT NOT NULL DEFAULT 'allow'",
            [],
        )?;
        added = true;
    }

    // One-time backfill for legacy boolean-only scopes.
    if added {
        conn.execute_batch(
            "UPDATE permission_scopes
               SET content_scan_mode = CASE WHEN allow_content_scan = 1 THEN 'allow' ELSE 'deny' END
             WHERE content_scan_mode = 'ask';
             UPDATE permission_scopes
               SET modification_mode = CASE WHEN allow_modification = 1 THEN 'allow' ELSE 'deny' END
             WHERE modification_mode = 'ask';
             UPDATE permission_scopes
               SET ocr_mode = CASE WHEN allow_ocr = 1 THEN 'allow' ELSE 'deny' END
             WHERE ocr_mode = 'allow' OR ocr_mode = 'ask';
             UPDATE permission_scopes
               SET indexing_mode = CASE WHEN allow_indexing = 1 THEN 'allow' ELSE 'deny' END
             WHERE indexing_mode = 'allow' OR indexing_mode = 'ask';",
        )?;
    }

    Ok(())
}

fn ensure_permission_default_settings(conn: &Connection) -> Result<(), AppError> {
    conn.execute(
        "INSERT OR IGNORE INTO settings(key, value) VALUES (?1, ?2)",
        ("permission_default_content_scan", "ask"),
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO settings(key, value) VALUES (?1, ?2)",
        ("permission_default_modification", "ask"),
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO settings(key, value) VALUES (?1, ?2)",
        ("permission_default_ocr", "ask"),
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO settings(key, value) VALUES (?1, ?2)",
        ("permission_default_indexing", "allow"),
    )?;
    Ok(())
}

pub fn run_migrations(conn: &Connection) -> Result<(), AppError> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.execute_batch(SCHEMA_V1)?;
    ensure_permission_mode_columns(conn)?;
    conn.execute_batch(SCHEMA_V2)?;
    conn.execute(SCHEMA_V2_VEC, [])?;
    conn.execute_batch(SCHEMA_V3)?;
    ensure_permission_default_settings(conn)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn register_vec_extension() {
        crate::data::register_sqlite_vec_extension();
    }

    fn test_conn() -> Connection {
        register_vec_extension();
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn test_migration_creates_tables() {
        let conn = test_conn();
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
        register_vec_extension();
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
        let conn = test_conn();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap();
    }

    #[test]
    fn test_migration_v2_creates_tables() {
        let conn = test_conn();
        run_migrations(&conn).unwrap();

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"ocr_text".to_string()));
        assert!(tables.contains(&"ai_metadata".to_string()));
        assert!(tables.contains(&"chat_history".to_string()));
        assert!(tables.contains(&"api_audit_log".to_string()));
        assert!(tables.contains(&"vec_index".to_string()));

        let has_fts: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE name = 'files_fts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(has_fts);
    }

    #[test]
    fn test_fts5_insert_and_search() {
        let conn = test_conn();
        run_migrations(&conn).unwrap();

        conn.execute(
            "INSERT INTO files_fts(file_path, file_name, ocr_text) VALUES (?1, ?2, ?3)",
            ["/home/user/readme.md", "readme.md", "setup instructions"],
        )
        .unwrap();

        let result: String = conn
            .query_row(
                "SELECT file_path FROM files_fts WHERE files_fts MATCH ?1",
                ["readme"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(result, "/home/user/readme.md");
    }

    fn f32_to_bytes(v: &[f32]) -> Vec<u8> {
        v.iter().flat_map(|f| f.to_le_bytes()).collect()
    }

    #[test]
    fn test_vec_index_insert_and_cosine_query() {
        let conn = test_conn();
        run_migrations(&conn).unwrap();

        let mut embedding = vec![0.0f32; 384];
        embedding[0] = 1.0;
        let bytes = f32_to_bytes(&embedding);

        conn.execute(
            "INSERT INTO vec_index(file_path, embedding) VALUES (?1, ?2)",
            rusqlite::params!["/test/file.txt", bytes],
        )
        .unwrap();

        let result: String = conn
            .query_row(
                "SELECT file_path FROM vec_index WHERE embedding MATCH ?1 AND k = 1",
                rusqlite::params![bytes],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(result, "/test/file.txt");
    }
}
