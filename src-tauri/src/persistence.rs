use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;

pub const CURRENT_SCHEMA_VERSION: i64 = 1;

pub fn open_database(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create database directory {}", parent.display()))?;
    }

    let mut conn = Connection::open(path)
        .with_context(|| format!("failed to open database {}", path.display()))?;
    configure_connection(&conn)?;
    apply_migrations(&mut conn)?;
    Ok(conn)
}

fn configure_connection(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "busy_timeout", 5000_i64)?;
    Ok(())
}

fn apply_migrations(conn: &mut Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );",
    )?;

    let current_version: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;

    if current_version < 1 {
        let tx = conn.transaction()?;
        tx.execute_batch(V1_SCHEMA)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
            (CURRENT_SCHEMA_VERSION, "phase_1_initial_state"),
        )?;
        tx.commit()?;
    }

    Ok(())
}

const V1_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS windows (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL UNIQUE,
    x REAL,
    y REAL,
    width REAL NOT NULL,
    height REAL NOT NULL,
    fullscreen INTEGER NOT NULL DEFAULT 0,
    maximized INTEGER NOT NULL DEFAULT 0,
    active_tab_id TEXT,
    sidebar_width REAL NOT NULL DEFAULT 240,
    sidebar_collapsed INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS tabs (
    id TEXT PRIMARY KEY,
    window_id TEXT NOT NULL REFERENCES windows(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    title TEXT NOT NULL,
    position INTEGER NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 0,
    view_mode TEXT NOT NULL DEFAULT 'list',
    sort_key TEXT NOT NULL DEFAULT 'name',
    sort_direction TEXT NOT NULL DEFAULT 'asc',
    folders_first INTEGER NOT NULL DEFAULT 1,
    hidden_files_visible INTEGER NOT NULL DEFAULT 0,
    file_extensions_visible INTEGER NOT NULL DEFAULT 0,
    scroll_offset REAL NOT NULL DEFAULT 0,
    selected_item_path TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_tabs_window_position ON tabs(window_id, position);
CREATE INDEX IF NOT EXISTS idx_tabs_path ON tabs(path);

CREATE TABLE IF NOT EXISTS folder_view_states (
    path TEXT PRIMARY KEY,
    view_mode TEXT NOT NULL DEFAULT 'list',
    sort_key TEXT NOT NULL DEFAULT 'name',
    sort_direction TEXT NOT NULL DEFAULT 'asc',
    folders_first INTEGER NOT NULL DEFAULT 1,
    hidden_files_visible INTEGER NOT NULL DEFAULT 0,
    file_extensions_visible INTEGER NOT NULL DEFAULT 0,
    scroll_offset REAL NOT NULL DEFAULT 0,
    selected_item_path TEXT,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS sidebar_sections (
    id TEXT PRIMARY KEY,
    visible INTEGER NOT NULL DEFAULT 1,
    position INTEGER NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS favorites (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    label TEXT NOT NULL,
    position INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS recents (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL,
    opened_at TEXT NOT NULL,
    open_count INTEGER NOT NULL DEFAULT 1,
    metadata_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_recents_opened_at ON recents(opened_at DESC);

CREATE TABLE IF NOT EXISTS metadata_index (
    path TEXT PRIMARY KEY,
    parent_path TEXT NOT NULL,
    name TEXT NOT NULL,
    display_name TEXT NOT NULL,
    kind TEXT NOT NULL,
    is_dir INTEGER NOT NULL,
    size INTEGER,
    modified_at TEXT,
    created_at TEXT,
    indexed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    hidden INTEGER NOT NULL DEFAULT 0,
    extension TEXT,
    search_text TEXT NOT NULL,
    recent_boost REAL NOT NULL DEFAULT 0,
    modified_boost REAL NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_metadata_parent_path ON metadata_index(parent_path);
CREATE INDEX IF NOT EXISTS idx_metadata_name ON metadata_index(name);
CREATE INDEX IF NOT EXISTS idx_metadata_kind ON metadata_index(kind);
CREATE INDEX IF NOT EXISTS idx_metadata_is_dir ON metadata_index(is_dir);
CREATE INDEX IF NOT EXISTS idx_metadata_search_text ON metadata_index(search_text);

CREATE TABLE IF NOT EXISTS index_state (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL,
    has_initial_index INTEGER NOT NULL DEFAULT 0,
    started_at TEXT,
    completed_at TEXT,
    checkpoint_json TEXT NOT NULL DEFAULT '{}',
    error_json TEXT,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS thumbnail_metadata (
    source_path TEXT PRIMARY KEY,
    thumbnail_path TEXT NOT NULL,
    source_modified_at TEXT,
    source_size INTEGER,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    generated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_accessed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_thumbnail_path ON thumbnail_metadata(thumbnail_path);
CREATE INDEX IF NOT EXISTS idx_thumbnail_last_accessed ON thumbnail_metadata(last_accessed_at);

CREATE TABLE IF NOT EXISTS activity_failures (
    id TEXT PRIMARY KEY,
    operation TEXT NOT NULL,
    path TEXT,
    message TEXT NOT NULL,
    recoverable INTEGER NOT NULL DEFAULT 0,
    failure_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    metadata_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_activity_failures_failure_at ON activity_failures(failure_at DESC);

INSERT OR IGNORE INTO settings (key, value) VALUES
    ('appearance.mode', 'system'),
    ('browser.hiddenFilesVisible', 'false'),
    ('browser.fileExtensionsVisible', 'false'),
    ('browser.foldersFirst', 'true'),
    ('browser.pathBarVisible', 'true'),
    ('restore.enabled', 'true'),
    ('privacy.localOnlyIndexing', 'true'),
    ('previews.enabled', 'true');

INSERT OR IGNORE INTO sidebar_sections (id, visible, position) VALUES
    ('recents', 1, 0),
    ('favorites', 1, 1),
    ('locations', 1, 2);

INSERT OR IGNORE INTO index_state (id, status, has_initial_index) VALUES
    ('metadata', 'not_started', 0);
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn test_database_path() -> PathBuf {
        std::env::temp_dir().join(format!("frogger-persistence-{}.sqlite3", Uuid::new_v4()))
    }

    #[test]
    fn fresh_migration_creates_required_tables() {
        let path = test_database_path();
        let conn = open_database(&path).expect("database should open and migrate");

        let tables = [
            "schema_migrations",
            "windows",
            "tabs",
            "folder_view_states",
            "settings",
            "sidebar_sections",
            "favorites",
            "recents",
            "metadata_index",
            "index_state",
            "thumbnail_metadata",
            "activity_failures",
        ];

        for table in tables {
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .expect("table lookup should succeed");
            assert_eq!(exists, 1, "missing table {table}");
        }

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn repeated_migration_is_idempotent() {
        let path = test_database_path();
        drop(open_database(&path).expect("first open should migrate"));
        let conn = open_database(&path).expect("second open should not remigrate destructively");

        let migration_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration count should be readable");
        assert_eq!(migration_count, 1);

        let version: i64 = conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration version should be readable");
        assert_eq!(version, CURRENT_SCHEMA_VERSION);

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn sample_insert_read_supports_settings_tabs_and_metadata() {
        let path = test_database_path();
        let conn = open_database(&path).expect("database should open and migrate");

        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params!["browser.hiddenFilesVisible", "true"],
        )
        .expect("setting should upsert");

        conn.execute(
            "INSERT INTO windows (id, label, width, height) VALUES (?1, ?2, ?3, ?4)",
            params!["window-1", "main", 1200.0_f64, 780.0_f64],
        )
        .expect("window should insert");

        conn.execute(
            "INSERT INTO tabs (id, window_id, path, title, position, is_active)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "tab-1",
                "window-1",
                "/Users/example",
                "example",
                0_i64,
                1_i64
            ],
        )
        .expect("tab should insert");

        conn.execute(
            "INSERT INTO metadata_index (
                path, parent_path, name, display_name, kind, is_dir, size, search_text
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                "/Users/example/file.txt",
                "/Users/example",
                "file.txt",
                "file",
                "Text Document",
                0_i64,
                42_i64,
                "file txt text document"
            ],
        )
        .expect("metadata row should insert");

        let hidden_files_visible: String = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'browser.hiddenFilesVisible'",
                [],
                |row| row.get(0),
            )
            .expect("setting should read");
        assert_eq!(hidden_files_visible, "true");

        let tab_path: String = conn
            .query_row("SELECT path FROM tabs WHERE id = 'tab-1'", [], |row| {
                row.get(0)
            })
            .expect("tab should read");
        assert_eq!(tab_path, "/Users/example");

        let indexed_kind: String = conn
            .query_row(
                "SELECT kind FROM metadata_index WHERE path = '/Users/example/file.txt'",
                [],
                |row| row.get(0),
            )
            .expect("metadata row should read");
        assert_eq!(indexed_kind, "Text Document");

        std::fs::remove_file(path).ok();
    }
}
