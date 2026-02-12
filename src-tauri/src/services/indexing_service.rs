use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify_debouncer_mini::notify;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, DebouncedEventKind};
use rusqlite::Connection;

use crate::data::repository;
use crate::error::AppError;
use crate::models::file_entry::FileEntry;
use crate::services::embedding_service;
use crate::services::ocr_service;

pub struct IndexingHandle {
    _debouncer: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
}

pub fn file_entry_from_path(path: &Path) -> Option<FileEntry> {
    let metadata = path.metadata().ok()?;
    let name = path.file_name()?.to_string_lossy().to_string();
    let extension = path.extension().map(|e| e.to_string_lossy().to_string());
    let mime_type = extension
        .as_ref()
        .and_then(|ext| mime_guess::from_ext(ext).first())
        .map(|m| m.to_string());
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let parent = canonical.parent().map(|p| p.to_string_lossy().to_string());

    Some(FileEntry {
        path: canonical.to_string_lossy().to_string(),
        name,
        extension,
        mime_type,
        size_bytes: Some(metadata.len() as i64),
        created_at: metadata
            .created()
            .ok()
            .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339()),
        modified_at: metadata
            .modified()
            .ok()
            .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339()),
        is_directory: metadata.is_dir(),
        parent_path: parent,
    })
}

pub fn process_event(conn: &Connection, path: &Path) {
    if path.exists() {
        if let Some(entry) = file_entry_from_path(path) {
            let _ = repository::insert_file(conn, &entry);
            if !entry.is_directory {
                let modified = entry.modified_at.as_deref().unwrap_or("");
                let ocr_text = ocr_service::process_file(conn, &entry.path, &entry.name, modified)
                    .unwrap_or(None)
                    .unwrap_or_default();
                let _ = repository::insert_fts(conn, &entry.path, &entry.name, &ocr_text);
                if !ocr_text.is_empty() {
                    let _ = embedding_service::embed_file(
                        conn,
                        &entry.path,
                        &entry.name,
                        entry.extension.as_deref(),
                        Some(ocr_text.as_str()),
                    );
                }
            }
        }
    } else {
        let path_str = path.to_string_lossy();
        let _ = repository::delete_file_index(conn, &path_str);
    }
}

pub fn start_watching(
    db: Arc<Mutex<Connection>>,
    directory: &str,
) -> Result<IndexingHandle, AppError> {
    let dir_path = Path::new(directory);
    if !dir_path.is_dir() {
        return Err(AppError::Watcher(format!("not a directory: {directory}")));
    }

    let db_clone = db.clone();
    let mut debouncer = new_debouncer(
        Duration::from_millis(500),
        move |result: DebounceEventResult| match result {
            Ok(events) => {
                let conn = db_clone.lock().unwrap();
                for event in events {
                    if matches!(
                        event.kind,
                        DebouncedEventKind::Any | DebouncedEventKind::AnyContinuous
                    ) {
                        process_event(&conn, &event.path);
                    }
                }
            }
            Err(e) => {
                eprintln!("watcher error: {e:?}");
            }
        },
    )
    .map_err(|e| AppError::Watcher(e.to_string()))?;

    debouncer
        .watcher()
        .watch(dir_path, notify::RecursiveMode::Recursive)
        .map_err(|e| AppError::Watcher(e.to_string()))?;

    Ok(IndexingHandle {
        _debouncer: debouncer,
    })
}

pub fn stop_watching(handle: IndexingHandle) {
    drop(handle);
}

#[cfg(test)]
pub fn scan_directory(conn: &Connection, directory: &str) {
    scan_directory_with_progress(conn, directory, |_, _| {});
}

const SKIP_DIRS: &[&str] = &[
    // System / OS
    "Library",
    "Applications",
    "System",
    "bin",
    "sbin",
    "usr",
    "var",
    "etc",
    "tmp",
    "private",
    // Build artifacts / dependencies
    "node_modules",
    "target",
    "build",
    "dist",
    "out",
    "__pycache__",
    ".venv",
    "venv",
    "Pods",
    // VCS / IDE
    ".git",
    ".svn",
    ".idea",
    ".vscode",
    // Caches
    "CachedData",
    "Cache",
    "Caches",
    "GPUCache",
    "ShaderCache",
    "Code Cache",
    "DerivedData",
];

const SKIP_EXTENSIONS: &[&str] = &[
    // Compiled / binary
    "o", "a", "dylib", "so", "dll", "exe", "class", "pyc", "pyo", "wasm",
    // Source maps / lockfiles / data
    "map", "lock", "bin", "dat", "db", "sqlite", "sqlite3", // Icons
    "ico", "icns", "cur", // Archives
    "zip", "tar", "gz", "bz2", "xz", "7z", "rar", "dmg", "iso", // Media
    "mp3", "mp4", "mov", "avi", "mkv", "flac", "wav", "m4a", "aac", "ogg", "m4v", "wmv",
    // Fonts
    "ttf", "otf", "woff", "woff2", "eot",
];

fn should_skip(entry: &walkdir::DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();

    if name.starts_with('.') {
        return true;
    }

    if entry.file_type().is_dir() {
        if SKIP_DIRS.iter().any(|&d| name == d) {
            return true;
        }
        if name.ends_with(".app")
            || name.ends_with(".framework")
            || name.ends_with(".bundle")
            || name.ends_with(".xcodeproj")
            || name.ends_with(".xcworkspace")
        {
            return true;
        }
    }

    if entry.file_type().is_file() {
        if let Some(ext) = Path::new(name.as_ref())
            .extension()
            .and_then(|e| e.to_str())
        {
            if SKIP_EXTENSIONS.contains(&ext) {
                return true;
            }
        }
    }

    false
}

pub fn scan_directory_with_progress<F>(conn: &Connection, directory: &str, on_progress: F)
where
    F: Fn(usize, usize),
{
    let dir = Path::new(directory);
    if !dir.is_dir() {
        return;
    }

    let walker = || {
        walkdir::WalkDir::new(dir)
            .min_depth(1)
            .max_depth(5)
            .into_iter()
            .filter_entry(|e| !should_skip(e))
            .filter_map(|e| e.ok())
    };

    let total = walker().count();
    let mut processed = 0usize;

    on_progress(0, total);

    let _ = conn.execute_batch("BEGIN");

    for entry in walker() {
        processed += 1;
        let path = entry.path();
        if let Some(file_entry) = file_entry_from_path(path) {
            let modified = file_entry.modified_at.as_deref().unwrap_or("");
            if !repository::needs_reindex(conn, &file_entry.path, modified) {
                if processed % 100 == 0 {
                    on_progress(processed, total);
                }
                continue;
            }
            process_event(conn, path);
        }
        if processed % 50 == 0 {
            let _ = conn.execute_batch("COMMIT");
            let _ = conn.execute_batch("BEGIN");
            on_progress(processed, total);
        }
    }

    let _ = conn.execute_batch("COMMIT");
    on_progress(total, total);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::migrations;
    use std::fs;
    use std::thread;

    fn test_conn() -> Connection {
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
        let conn = Connection::open_in_memory().unwrap();
        migrations::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn test_file_entry_from_path() {
        let dir = std::env::temp_dir().join("frogger_test_entry_from_path");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("hello.txt"), "world").unwrap();

        let entry = file_entry_from_path(&dir.join("hello.txt")).unwrap();
        assert_eq!(entry.name, "hello.txt");
        assert_eq!(entry.extension.as_deref(), Some("txt"));
        assert_eq!(entry.size_bytes, Some(5));
        assert!(!entry.is_directory);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_file_entry_from_path_dir() {
        let dir = std::env::temp_dir().join("frogger_test_entry_dir");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let entry = file_entry_from_path(&dir).unwrap();
        assert!(entry.is_directory);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_file_entry_from_nonexistent() {
        let result = file_entry_from_path(Path::new("/tmp/nonexistent_frogger_abc123"));
        assert!(result.is_none());
    }

    fn poll_until<F>(db: &Arc<Mutex<Connection>>, timeout_ms: u64, check: F) -> bool
    where
        F: Fn(&Connection) -> bool,
    {
        let start = std::time::Instant::now();
        let timeout = Duration::from_millis(timeout_ms);
        while start.elapsed() < timeout {
            {
                let conn = db.lock().unwrap();
                if check(&conn) {
                    return true;
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
        false
    }

    fn canonical(p: &Path) -> String {
        p.canonicalize()
            .unwrap_or_else(|_| p.to_path_buf())
            .to_string_lossy()
            .to_string()
    }

    #[test]
    fn test_watcher_detects_file_create() {
        let dir = std::env::temp_dir().join("frogger_test_watcher_create");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let conn = test_conn();
        let db = Arc::new(Mutex::new(conn));
        let handle = start_watching(db.clone(), dir.to_str().unwrap()).unwrap();

        let file = dir.join("watched.txt");
        fs::write(&file, "hello").unwrap();

        let path_str = canonical(&file);
        let found = poll_until(&db, 5000, |conn| {
            repository::get_by_path(conn, &path_str).unwrap().is_some()
        });

        stop_watching(handle);
        let _ = fs::remove_dir_all(&dir);

        assert!(
            found,
            "watcher should detect file creation and insert into DB"
        );
    }

    #[test]
    fn test_watcher_detects_file_delete() {
        let dir = std::env::temp_dir().join("frogger_test_watcher_delete");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let file_path = dir.join("to_delete.txt");
        fs::write(&file_path, "goodbye").unwrap();

        let conn = test_conn();
        let entry = file_entry_from_path(&file_path).unwrap();
        repository::insert_file(&conn, &entry).unwrap();

        let db = Arc::new(Mutex::new(conn));
        let handle = start_watching(db.clone(), dir.to_str().unwrap()).unwrap();

        fs::remove_file(&file_path).unwrap();

        let path_str = canonical(&dir).to_string() + "/to_delete.txt";
        let removed = poll_until(&db, 5000, |conn| {
            repository::get_by_path(conn, &path_str).unwrap().is_none()
        });

        stop_watching(handle);
        let _ = fs::remove_dir_all(&dir);

        assert!(
            removed,
            "watcher should detect file deletion and remove from DB"
        );
    }

    #[test]
    fn test_watcher_invalid_directory() {
        let conn = test_conn();
        let db = Arc::new(Mutex::new(conn));
        let result = start_watching(db, "/nonexistent/dir/frogger_xyz");
        assert!(result.is_err());
    }

    #[test]
    fn test_incremental_reindex() {
        let dir = std::env::temp_dir().join("frogger_test_reindex");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let conn = test_conn();
        let db = Arc::new(Mutex::new(conn));
        let handle = start_watching(db.clone(), dir.to_str().unwrap()).unwrap();

        let file_path = dir.join("data.txt");
        fs::write(&file_path, "v1").unwrap();

        let path_str = canonical(&file_path);

        let created = poll_until(&db, 5000, |conn| {
            repository::get_by_path(conn, &path_str).unwrap().is_some()
        });
        assert!(created, "file should be indexed after create");

        {
            let conn = db.lock().unwrap();
            let entry = repository::get_by_path(&conn, &path_str).unwrap().unwrap();
            assert_eq!(entry.size_bytes, Some(2));
        }

        fs::write(&file_path, "version two content").unwrap();

        let updated = poll_until(&db, 5000, |conn| {
            repository::get_by_path(conn, &path_str)
                .unwrap()
                .map(|e| e.size_bytes == Some(19))
                .unwrap_or(false)
        });

        stop_watching(handle);
        let _ = fs::remove_dir_all(&dir);

        assert!(updated, "file size should update after modify");
    }

    #[test]
    fn test_scan_directory_indexes_new_files() {
        let dir = std::env::temp_dir().join("frogger_test_scan_new");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("a.txt"), "aaa").unwrap();
        fs::write(dir.join("sub/b.txt"), "bbb").unwrap();

        let conn = test_conn();
        scan_directory(&conn, dir.to_str().unwrap());

        let a_path = canonical(&dir.join("a.txt"));
        let b_path = canonical(&dir.join("sub/b.txt"));

        assert!(
            repository::get_by_path(&conn, &a_path).unwrap().is_some(),
            "a.txt should be indexed"
        );
        assert!(
            repository::get_by_path(&conn, &b_path).unwrap().is_some(),
            "sub/b.txt should be indexed"
        );

        let fts = repository::search_fts(&conn, "a", 10).unwrap();
        assert!(!fts.is_empty(), "FTS should find a.txt");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_directory_skips_unchanged() {
        let dir = std::env::temp_dir().join("frogger_test_scan_skip");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("existing.txt"), "hello").unwrap();

        let conn = test_conn();

        // First scan indexes the file
        scan_directory(&conn, dir.to_str().unwrap());

        let path_str = canonical(&dir.join("existing.txt"));
        let entry = repository::get_by_path(&conn, &path_str).unwrap().unwrap();
        assert_eq!(entry.size_bytes, Some(5));

        // FTS count should be 1
        let fts_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM files_fts WHERE file_path = ?1",
                rusqlite::params![path_str],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(fts_count, 1);

        // Second scan should skip (file unchanged)
        scan_directory(&conn, dir.to_str().unwrap());

        // FTS count should still be 1 (not duplicated)
        let fts_count_after: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM files_fts WHERE file_path = ?1",
                rusqlite::params![path_str],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(fts_count_after, 1);

        let _ = fs::remove_dir_all(&dir);
    }
}
