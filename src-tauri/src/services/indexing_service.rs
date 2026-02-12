use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
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
use crate::services::permission_service::{self, PermissionCapability};
use crate::services::spreadsheet_service;

pub struct IndexingHandle {
    _debouncer: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
    stop_flag: Arc<AtomicBool>,
    poller: Option<std::thread::JoinHandle<()>>,
    enrichment_worker: Option<std::thread::JoinHandle<()>>,
    bootstrap_worker: Option<std::thread::JoinHandle<()>>,
    enrichment_queue: EnrichmentQueue,
}

#[derive(Clone)]
pub(crate) struct EnrichmentQueue {
    sender: Sender<String>,
    pending: Arc<Mutex<HashSet<String>>>,
}

impl IndexingHandle {
    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        self.stop_flag.clone()
    }

    pub fn enrichment_queue(&self) -> EnrichmentQueue {
        self.enrichment_queue.clone()
    }

    pub fn attach_bootstrap_worker(&mut self, worker: std::thread::JoinHandle<()>) {
        self.bootstrap_worker = Some(worker);
    }
}

const POLLER_FULL_RECONCILE_INTERVAL_TICKS: u32 = 300;
const POLLER_INTERVAL_MS: u64 = 3000;
const WATCH_DEBOUNCE_MS: u64 = 150;

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

fn process_event_with_default(
    conn: &Connection,
    path: &Path,
    scopes: &[repository::PermissionScope],
    default_mode: permission_service::PermissionMode,
    allow_once: bool,
    enrichment_queue: Option<&EnrichmentQueue>,
) {
    if !path.exists() {
        let path_str = path.to_string_lossy();
        let _ = repository::delete_file_index(conn, &path_str);
        return;
    }

    let Some(entry) = file_entry_from_path(path) else {
        return;
    };

    if permission_service::enforce_with_scopes(
        scopes,
        &entry.path,
        PermissionCapability::Indexing,
        allow_once,
        default_mode,
    )
    .is_err()
    {
        return;
    }

    if !entry.is_directory {
        let modified = entry.modified_at.as_deref().unwrap_or("");
        if !repository::needs_reindex(conn, &entry.path, modified) {
            return;
        }
    }

    let _ = repository::insert_file(conn, &entry);
    if entry.is_directory {
        return;
    }

    if has_skip_extension(path) {
        let _ = repository::insert_fts(conn, &entry.path, &entry.name, "");
        return;
    }

    if let Some(queue) = enrichment_queue {
        let _ = repository::insert_fts(conn, &entry.path, &entry.name, "");
        enqueue_enrichment(queue, &entry.path);
        return;
    }

    enrich_entry_with_default(conn, &entry, allow_once);
}

fn enrich_entry_with_default(conn: &Connection, entry: &FileEntry, allow_once: bool) {
    let modified = entry.modified_at.as_deref().unwrap_or("");
    let mut index_text = ocr_service::process_file(conn, &entry.path, &entry.name, modified)
        .unwrap_or(None)
        .unwrap_or_default();

    if index_text.is_empty() {
        index_text = spreadsheet_service::extract_text(conn, &entry.path, allow_once)
            .unwrap_or(None)
            .unwrap_or_default();

        if index_text.is_empty() {
            let _ = repository::delete_ocr_text(conn, &entry.path);
        } else {
            let now = chrono::Utc::now().to_rfc3339();
            let _ =
                repository::insert_ocr_text(conn, &entry.path, &index_text, "sheet", None, &now);
        }
    }

    let _ = repository::insert_fts(conn, &entry.path, &entry.name, &index_text);
    if !index_text.is_empty() {
        let _ = embedding_service::embed_file(
            conn,
            &entry.path,
            &entry.name,
            entry.extension.as_deref(),
            Some(index_text.as_str()),
        );
    }
}

fn enqueue_enrichment(queue: &EnrichmentQueue, path: &str) {
    {
        let mut pending = queue
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !pending.insert(path.to_string()) {
            return;
        }
    }

    if let Err(err) = queue.sender.send(path.to_string()) {
        let mut pending = queue
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        pending.remove(&err.0);
    }
}

fn canonical(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

fn reconcile_directory_with_default(
    conn: &Connection,
    dir: &Path,
    scopes: &[repository::PermissionScope],
    default_mode: permission_service::PermissionMode,
    allow_once: bool,
    enrichment_queue: Option<&EnrichmentQueue>,
) {
    if !dir.is_dir() {
        return;
    }

    let parent = canonical(dir);
    let indexed = repository::list_by_parent(conn, &parent).unwrap_or_default();

    let mut on_disk: HashSet<String> = HashSet::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            on_disk.insert(canonical(&p));
            process_event_with_default(
                conn,
                &p,
                scopes,
                default_mode,
                allow_once,
                enrichment_queue,
            );
        }
    }

    for entry in indexed {
        if !on_disk.contains(&entry.path) {
            let _ = repository::delete_file_index(conn, &entry.path);
        }
    }
}

fn should_skip_reconcile_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    name.starts_with('.') || SKIP_DIRS.contains(&name)
}

fn reconcile_tree_with_default(
    conn: &Connection,
    root: &Path,
    scopes: &[repository::PermissionScope],
    default_mode: permission_service::PermissionMode,
    allow_once: bool,
    enrichment_queue: Option<&EnrichmentQueue>,
) {
    if !root.is_dir() {
        return;
    }

    let mut stack = vec![root.to_path_buf()];
    let mut visited: HashSet<String> = HashSet::new();

    while let Some(dir) = stack.pop() {
        let dir_key = canonical(&dir);
        if !visited.insert(dir_key) {
            continue;
        }

        reconcile_directory_with_default(
            conn,
            &dir,
            scopes,
            default_mode,
            allow_once,
            enrichment_queue,
        );

        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let child = entry.path();
                if child.is_dir() && !should_skip_reconcile_dir(&child) {
                    stack.push(child);
                }
            }
        }
    }
}

fn run_enrichment_worker(
    db_path: String,
    receiver: mpsc::Receiver<String>,
    pending: Arc<Mutex<HashSet<String>>>,
    stop_flag: Arc<AtomicBool>,
    allow_once: bool,
) {
    let conn = match Connection::open(&db_path) {
        Ok(conn) => conn,
        Err(err) => {
            sentry::capture_message(
                &format!("indexing enrichment worker failed to open db: {err}"),
                sentry::Level::Error,
            );
            return;
        }
    };

    let _ = conn.busy_timeout(Duration::from_secs(5));

    while !stop_flag.load(Ordering::Relaxed) {
        let path = match receiver.recv_timeout(Duration::from_millis(250)) {
            Ok(path) => path,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };

        {
            let mut guard = pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.remove(&path);
        }

        let path_buf = Path::new(&path);
        if !path_buf.exists() || path_buf.is_dir() || has_skip_extension(path_buf) {
            continue;
        }

        let Some(entry) = file_entry_from_path(path_buf) else {
            continue;
        };

        let scopes = repository::get_permission_scopes(&conn).unwrap_or_default();
        let default_mode =
            permission_service::resolve_default_mode(&conn, PermissionCapability::Indexing)
                .unwrap_or(permission_service::PermissionMode::Allow);
        if permission_service::enforce_with_scopes(
            &scopes,
            &entry.path,
            PermissionCapability::Indexing,
            allow_once,
            default_mode,
        )
        .is_err()
        {
            continue;
        }

        enrich_entry_with_default(&conn, &entry, allow_once);
    }
}

pub fn start_watching(
    db: Arc<Mutex<Connection>>,
    db_path: String,
    directory: &str,
    allow_once: bool,
) -> Result<IndexingHandle, AppError> {
    let dir_path = Path::new(directory);
    if !dir_path.is_dir() {
        return Err(AppError::Watcher(format!("not a directory: {directory}")));
    }

    let (enrichment_sender, enrichment_receiver) = mpsc::channel();
    let enrichment_pending = Arc::new(Mutex::new(HashSet::new()));
    let enrichment_queue = EnrichmentQueue {
        sender: enrichment_sender,
        pending: enrichment_pending.clone(),
    };

    let db_clone = db.clone();
    let watcher_enrichment_queue = enrichment_queue.clone();
    let watched_dir = dir_path.to_path_buf();
    let mut debouncer = new_debouncer(
        Duration::from_millis(WATCH_DEBOUNCE_MS),
        move |result: DebounceEventResult| match result {
            Ok(events) => {
                let conn = db_clone.lock().unwrap();
                let scopes = repository::get_permission_scopes(&conn).unwrap_or_default();
                let default_mode =
                    permission_service::resolve_default_mode(&conn, PermissionCapability::Indexing)
                        .unwrap_or(permission_service::PermissionMode::Allow);
                for event in events {
                    if matches!(
                        event.kind,
                        DebouncedEventKind::Any | DebouncedEventKind::AnyContinuous
                    ) {
                        if event.path.is_dir() {
                            reconcile_directory_with_default(
                                &conn,
                                &event.path,
                                &scopes,
                                default_mode,
                                allow_once,
                                Some(&watcher_enrichment_queue),
                            );
                        } else {
                            let path_exists = event.path.exists();
                            process_event_with_default(
                                &conn,
                                &event.path,
                                &scopes,
                                default_mode,
                                allow_once,
                                Some(&watcher_enrichment_queue),
                            );
                            if !path_exists {
                                if let Some(parent) = event.path.parent() {
                                    reconcile_directory_with_default(
                                        &conn,
                                        parent,
                                        &scopes,
                                        default_mode,
                                        allow_once,
                                        Some(&watcher_enrichment_queue),
                                    );
                                }
                            }
                        }
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

    // Fallback poller: keeps index fresh even if native watcher events are dropped.
    let stop_flag = Arc::new(AtomicBool::new(false));
    let enrichment_worker = if db_path == ":memory:" {
        None
    } else {
        let enrichment_stop = stop_flag.clone();
        let enrichment_pending_for_worker = enrichment_pending;
        Some(std::thread::spawn(move || {
            run_enrichment_worker(
                db_path,
                enrichment_receiver,
                enrichment_pending_for_worker,
                enrichment_stop,
                allow_once,
            );
        }))
    };

    let stop_clone = stop_flag.clone();
    let poll_db = db.clone();
    let poll_dir = watched_dir.clone();
    let poll_enrichment_queue = enrichment_queue.clone();
    let poller = std::thread::spawn(move || {
        let mut tick = 1u32;
        while !stop_clone.load(Ordering::Relaxed) {
            if let Ok(conn) = poll_db.try_lock() {
                let scopes = repository::get_permission_scopes(&conn).unwrap_or_default();
                let default_mode =
                    permission_service::resolve_default_mode(&conn, PermissionCapability::Indexing)
                        .unwrap_or(permission_service::PermissionMode::Allow);
                if tick.is_multiple_of(POLLER_FULL_RECONCILE_INTERVAL_TICKS) {
                    reconcile_tree_with_default(
                        &conn,
                        &poll_dir,
                        &scopes,
                        default_mode,
                        allow_once,
                        Some(&poll_enrichment_queue),
                    );
                } else {
                    reconcile_directory_with_default(
                        &conn,
                        &poll_dir,
                        &scopes,
                        default_mode,
                        allow_once,
                        Some(&poll_enrichment_queue),
                    );
                }
            }
            tick = tick.wrapping_add(1);
            std::thread::sleep(Duration::from_millis(POLLER_INTERVAL_MS));
        }
    });

    Ok(IndexingHandle {
        _debouncer: debouncer,
        stop_flag,
        poller: Some(poller),
        enrichment_worker,
        bootstrap_worker: None,
        enrichment_queue,
    })
}

pub fn stop_watching(mut handle: IndexingHandle) {
    handle.stop_flag.store(true, Ordering::Relaxed);
    if let Some(join) = handle.bootstrap_worker.take() {
        let _ = join.join();
    }
    if let Some(join) = handle.poller.take() {
        let _ = join.join();
    }
    if let Some(join) = handle.enrichment_worker.take() {
        let _ = join.join();
    }
}

#[cfg(test)]
pub fn scan_directory(conn: &Connection, directory: &str) {
    let _ = scan_directory_internal(conn, directory, false, Some(5), None, None, |_, _| {});
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

fn should_skip_dir(entry: &walkdir::DirEntry) -> bool {
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

    false
}

fn has_skip_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| SKIP_EXTENSIONS.contains(&ext))
}

fn scan_directory_internal<F>(
    conn: &Connection,
    directory: &str,
    allow_once: bool,
    max_depth: Option<usize>,
    cancel_flag: Option<&AtomicBool>,
    enrichment_queue: Option<&EnrichmentQueue>,
    on_progress: F,
) -> bool
where
    F: Fn(usize, usize),
{
    let dir = Path::new(directory);
    if !dir.is_dir() {
        return false;
    }

    let walker = || {
        let mut walk = walkdir::WalkDir::new(dir).min_depth(1);
        if let Some(depth) = max_depth {
            walk = walk.max_depth(depth);
        }
        walk.into_iter()
            .filter_entry(|entry| !should_skip_dir(entry))
            .filter_map(|entry| entry.ok())
    };

    let total = walker().count();
    let scopes = repository::get_permission_scopes(conn).unwrap_or_default();
    let default_mode =
        permission_service::resolve_default_mode(conn, PermissionCapability::Indexing)
            .unwrap_or(permission_service::PermissionMode::Allow);
    let mut processed = 0usize;

    on_progress(0, total);
    if cancel_flag.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
        return true;
    }

    let _ = conn.execute_batch("BEGIN");

    for entry in walker() {
        if cancel_flag.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            let _ = conn.execute_batch("COMMIT");
            on_progress(processed, total);
            return true;
        }

        processed += 1;
        let path = entry.path();
        process_event_with_default(
            conn,
            path,
            &scopes,
            default_mode,
            allow_once,
            enrichment_queue,
        );
        if processed.is_multiple_of(50) {
            let _ = conn.execute_batch("COMMIT");
            if cancel_flag.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                on_progress(processed, total);
                return true;
            }
            let _ = conn.execute_batch("BEGIN");
            on_progress(processed, total);
        }
    }

    let _ = conn.execute_batch("COMMIT");
    on_progress(total, total);
    false
}

pub fn scan_directory_deep_with_progress_cancel<F>(
    conn: &Connection,
    directory: &str,
    allow_once: bool,
    cancel_flag: &AtomicBool,
    on_progress: F,
) -> bool
where
    F: Fn(usize, usize),
{
    scan_directory_internal(
        conn,
        directory,
        allow_once,
        None,
        Some(cancel_flag),
        None,
        on_progress,
    )
}

pub fn scan_directory_deep_with_progress_cancel_deferred<F>(
    conn: &Connection,
    directory: &str,
    allow_once: bool,
    cancel_flag: &AtomicBool,
    enrichment_queue: &EnrichmentQueue,
    on_progress: F,
) -> bool
where
    F: Fn(usize, usize),
{
    scan_directory_internal(
        conn,
        directory,
        allow_once,
        None,
        Some(cancel_flag),
        Some(enrichment_queue),
        on_progress,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::migrations;
    use std::fs;
    use std::sync::atomic::AtomicBool;
    use std::thread;

    fn test_conn() -> Connection {
        crate::data::register_sqlite_vec_extension();
        let conn = Connection::open_in_memory().unwrap();
        migrations::run_migrations(&conn).unwrap();
        repository::set_setting(&conn, "permission_default_content_scan", "allow").unwrap();
        repository::set_setting(&conn, "permission_default_modification", "allow").unwrap();
        repository::set_setting(&conn, "permission_default_ocr", "allow").unwrap();
        repository::set_setting(&conn, "permission_default_indexing", "allow").unwrap();
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
        let handle = start_watching(
            db.clone(),
            ":memory:".to_string(),
            dir.to_str().unwrap(),
            false,
        )
        .unwrap();

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
        let handle = start_watching(
            db.clone(),
            ":memory:".to_string(),
            dir.to_str().unwrap(),
            false,
        )
        .unwrap();

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
        let result = start_watching(
            db,
            ":memory:".to_string(),
            "/nonexistent/dir/frogger_xyz",
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_bootstrap_scan_indexes_existing_nested_files() {
        let dir = std::env::temp_dir().join("frogger_test_watcher_initial_recursive_scan");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("nested/deeper")).unwrap();

        let file_path = dir.join("nested/deeper/seed.txt");
        fs::write(&file_path, "seed content").unwrap();

        let conn = test_conn();
        let db = Arc::new(Mutex::new(conn));
        let handle = start_watching(
            db.clone(),
            ":memory:".to_string(),
            dir.to_str().unwrap(),
            false,
        )
        .unwrap();
        let cancel = AtomicBool::new(false);

        {
            let conn = db.lock().unwrap();
            let cancelled = scan_directory_deep_with_progress_cancel(
                &conn,
                dir.to_str().unwrap(),
                false,
                &cancel,
                |_, _| {},
            );
            assert!(!cancelled, "bootstrap scan should complete");
        }

        let file_key = canonical(&file_path);
        let found = poll_until(&db, 5000, |conn| {
            repository::get_by_path(conn, &file_key).unwrap().is_some()
        });

        stop_watching(handle);
        let _ = fs::remove_dir_all(&dir);

        assert!(
            found,
            "bootstrap scan should index existing nested files recursively"
        );
    }

    #[test]
    fn test_incremental_reindex() {
        let dir = std::env::temp_dir().join("frogger_test_reindex");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let conn = test_conn();
        let db = Arc::new(Mutex::new(conn));
        let handle = start_watching(
            db.clone(),
            ":memory:".to_string(),
            dir.to_str().unwrap(),
            false,
        )
        .unwrap();

        let file_path = dir.join("data.txt");
        fs::write(&file_path, "v1").unwrap();

        let path_str = canonical(&file_path);

        let created = poll_until(&db, 5000, |conn| {
            repository::get_by_path(conn, &path_str)
                .unwrap()
                .map(|entry| entry.size_bytes == Some(2))
                .unwrap_or(false)
        });
        assert!(created, "file should be indexed after create");

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

    #[test]
    fn test_scan_indexes_skipped_extension_by_filename() {
        let dir = std::env::temp_dir().join("frogger_test_scan_skip_ext");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("song.mp3"), &[0u8; 64]).unwrap();
        fs::write(dir.join("archive.zip"), &[0u8; 64]).unwrap();
        fs::write(dir.join("notes.txt"), "hello").unwrap();

        let conn = test_conn();
        scan_directory(&conn, dir.to_str().unwrap());

        let fts_mp3 = repository::search_fts(&conn, "song", 10).unwrap();
        assert!(!fts_mp3.is_empty(), "mp3 should be searchable by filename");

        let fts_zip = repository::search_fts(&conn, "archive", 10).unwrap();
        assert!(!fts_zip.is_empty(), "zip should be searchable by filename");

        let fts_txt = repository::search_fts(&conn, "notes", 10).unwrap();
        assert!(!fts_txt.is_empty(), "txt should be searchable by filename");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_extracts_csv_content_for_fts_and_manifest_snippet() {
        let dir = std::env::temp_dir().join("frogger_test_scan_csv_content");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let csv_path = dir.join("finance.csv");
        fs::write(&csv_path, "project,amount\nalpha,1000\nbeta,2000\n").unwrap();

        let conn = test_conn();
        scan_directory(&conn, dir.to_str().unwrap());

        let fts_hits = repository::search_fts(&conn, "alpha", 10).unwrap();
        assert!(
            !fts_hits.is_empty(),
            "csv cell content should be searchable in FTS"
        );

        let csv_key = canonical(&csv_path);
        let snippet = repository::get_ocr_text(&conn, &csv_key).unwrap();
        assert!(
            snippet
                .as_ref()
                .map(|record| record.text_content.contains("alpha"))
                .unwrap_or(false),
            "csv content should be persisted for organize snippets"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
