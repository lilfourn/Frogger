use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
const EMBEDDING_CLEANUP_INTERVAL_TICKS: u32 = 300;
const POLLER_INTERVAL_MS: u64 = 3000;
const WATCH_DEBOUNCE_MS: u64 = 150;
const ENRICHMENT_EMBED_BATCH_SIZE: usize = 64;
const USER_INTERACTION_PAUSE_MS: i64 = 1500;
const USER_INTERACTION_PAUSE_SLEEP_MS: u64 = 120;
pub const VEC_RETENTION_DAYS: i64 = 7;
const HOME_ALLOWED_ROOT_NAMES: &[&str] = &[
    "Documents",
    "Desktop",
    "Downloads",
    "Pictures",
    "Movies",
    "Music",
];
#[cfg(target_os = "windows")]
const SYSTEM_BLOCKED_ROOTS: &[&str] = &[
    "C:/Windows",
    "C:/Program Files",
    "C:/Program Files (x86)",
    "C:/ProgramData",
];
#[cfg(not(target_os = "windows"))]
const SYSTEM_BLOCKED_ROOTS: &[&str] = &[
    "/System",
    "/Library",
    "/Applications",
    "/private",
    "/usr",
    "/bin",
    "/sbin",
    "/etc",
    "/var",
    "/dev",
    "/opt",
];

#[derive(Clone, Debug)]
struct IndexScopePolicy {
    scan_root: String,
    allowed_roots: Vec<String>,
    blocked_roots: Vec<String>,
}

impl IndexScopePolicy {
    fn build(directory: &str) -> Self {
        let scan_root_path = Path::new(directory);
        let scan_root = canonical(scan_root_path);
        let home_root = dirs::home_dir().map(|path| canonical(path.as_path()));
        let mut allowed_roots = Vec::new();
        let mut blocked_roots = SYSTEM_BLOCKED_ROOTS
            .iter()
            .map(|root| scope_path::normalize(root))
            .collect::<Vec<_>>();

        if home_root.as_deref() == Some(scan_root.as_str()) {
            let home_path = PathBuf::from(&scan_root);
            for name in HOME_ALLOWED_ROOT_NAMES {
                let candidate = home_path.join(name);
                if candidate.is_dir() {
                    allowed_roots.push(canonical(&candidate));
                }
            }
        } else {
            allowed_roots.push(scan_root.clone());
        }

        if allowed_roots.is_empty() {
            allowed_roots.push(scan_root.clone());
        }

        blocked_roots.retain(|root| {
            let normalized_root = scope_path::normalize(root);
            !scope_path::is_within_scope(&scan_root, &normalized_root)
                || scan_root == normalized_root
        });

        allowed_roots.sort();
        allowed_roots.dedup();
        blocked_roots.sort();
        blocked_roots.dedup();

        Self {
            scan_root,
            allowed_roots,
            blocked_roots,
        }
    }

    fn allowed_roots(&self) -> &[String] {
        &self.allowed_roots
    }

    fn blocked_roots(&self) -> &[String] {
        &self.blocked_roots
    }

    fn is_path_blocked_str(&self, path: &str) -> bool {
        if path_has_skipped_component_under_scan_root(path, &self.scan_root, true) {
            return true;
        }

        self.blocked_roots
            .iter()
            .any(|root| scope_path::is_within_scope(path, root))
    }

    fn is_path_blocked(&self, path: &Path, include_self: bool) -> bool {
        let canonical_path = canonical(path);
        if path_has_skipped_component_under_scan_root(
            &canonical_path,
            &self.scan_root,
            include_self,
        ) {
            return true;
        }
        self.blocked_roots
            .iter()
            .any(|root| scope_path::is_within_scope(&canonical_path, root))
    }

    fn is_path_allowed(&self, path: &Path) -> bool {
        let path = canonical(path);
        self.is_path_allowed_str(&path)
    }

    fn is_path_allowed_str(&self, path: &str) -> bool {
        if self.is_path_blocked_str(path) {
            return false;
        }

        self.allowed_roots
            .iter()
            .any(|root| scope_path::is_within_scope(path, root))
    }

    fn should_descend_dir(&self, path: &Path) -> bool {
        if self.is_path_blocked(path, true) {
            return false;
        }

        let dir = canonical(path);
        if !scope_path::is_within_scope(&dir, &self.scan_root) {
            return false;
        }

        self.allowed_roots.iter().any(|root| {
            scope_path::is_within_scope(&dir, root) || scope_path::is_within_scope(root, &dir)
        })
    }
}

pub fn resolve_allowed_roots_for_directory(directory: &str) -> Vec<String> {
    IndexScopePolicy::build(directory).allowed_roots().to_vec()
}

pub fn resolve_blocked_roots_for_directory(directory: &str) -> Vec<String> {
    IndexScopePolicy::build(directory).blocked_roots().to_vec()
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

fn process_event_with_default(
    conn: &Connection,
    path: &Path,
    scope_policy: &IndexScopePolicy,
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

    if should_skip_path(path, path.is_dir(), scope_policy) {
        let canonical_path = canonical(path);
        if path.is_dir() {
            let _ = repository::delete_file_index_subtree(conn, &canonical_path);
        } else {
            let _ = repository::delete_file_index(conn, &canonical_path);
        }
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
    if let Some(doc) = build_embedding_document(conn, entry, allow_once) {
        let _ = embedding_service::embed_documents(conn, &[doc]);
    }
}

fn build_embedding_document(
    conn: &Connection,
    entry: &FileEntry,
    allow_once: bool,
) -> Option<embedding_service::EmbeddingDocument> {
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
    if index_text.is_empty() {
        return None;
    }

    Some(embedding_service::EmbeddingDocument {
        file_path: entry.path.clone(),
        file_name: entry.name.clone(),
        extension: entry.extension.clone(),
        ocr_text: Some(index_text),
    })
}

fn flush_embedding_batch(conn: &Connection, batch: &mut Vec<embedding_service::EmbeddingDocument>) {
    if batch.is_empty() {
        return;
    }

    let docs = std::mem::take(batch);
    if let Err(err) = embedding_service::embed_documents(conn, &docs) {
        sentry::capture_message(
            &format!("indexing enrichment embedding batch failed: {err}"),
            sentry::Level::Warning,
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
    dunce::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

fn should_yield_for_user(last_user_interaction_at: Option<&Arc<AtomicI64>>) -> bool {
    let Some(last_user_interaction_at) = last_user_interaction_at else {
        return false;
    };
    let last = last_user_interaction_at.load(Ordering::Relaxed);
    if last <= 0 {
        return false;
    }

    chrono::Utc::now().timestamp_millis().saturating_sub(last) < USER_INTERACTION_PAUSE_MS
}

use crate::scope_path;

fn is_skip_bundle_name(name: &str) -> bool {
    name.ends_with(".app")
        || name.ends_with(".framework")
        || name.ends_with(".bundle")
        || name.ends_with(".xcodeproj")
        || name.ends_with(".xcworkspace")
}

fn is_system_dir_name(name: &str) -> bool {
    matches!(
        name,
        "Library"
            | "Applications"
            | "System"
            | "bin"
            | "sbin"
            | "usr"
            | "var"
            | "etc"
            | "tmp"
            | "private"
            | "dev"
            | "opt"
            | "Windows"
            | "Program Files"
            | "Program Files (x86)"
            | "ProgramData"
    )
}

fn is_skip_dir_name(name: &str) -> bool {
    name.starts_with('.') || SKIP_DIRS.contains(&name) || is_skip_bundle_name(name)
}

fn path_has_skipped_component_under_scan_root(
    path: &str,
    scan_root: &str,
    include_self: bool,
) -> bool {
    let normalized_path = scope_path::normalize(path);
    let normalized_root = scope_path::normalize(scan_root);
    let root_name = Path::new(&normalized_root)
        .file_name()
        .and_then(|value| value.to_str());
    if root_name.is_some_and(|name| {
        name.starts_with('.') || is_system_dir_name(name) || is_skip_bundle_name(name)
    }) {
        return true;
    }

    if normalized_path == normalized_root {
        if include_self {
            return root_name.is_some_and(is_skip_dir_name);
        }
        return false;
    }

    if !scope_path::is_within_scope(&normalized_path, &normalized_root) {
        return false;
    }

    let relative = if normalized_root == "/" {
        normalized_path.trim_start_matches('/')
    } else {
        normalized_path
            .strip_prefix(&(normalized_root + "/"))
            .unwrap_or_default()
    };

    let mut components = relative
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    if components
        .first()
        .is_some_and(|name| is_system_dir_name(name))
    {
        return true;
    }

    if !include_self {
        let _ = components.pop();
    }

    components.into_iter().any(is_skip_dir_name)
}

fn should_skip_path(path: &Path, is_dir: bool, scope_policy: &IndexScopePolicy) -> bool {
    if is_dir {
        !scope_policy.should_descend_dir(path)
    } else {
        !scope_policy.is_path_allowed(path)
            || path_has_skipped_component_under_scan_root(
                &canonical(path),
                &scope_policy.scan_root,
                false,
            )
    }
}

fn reconcile_directory_with_default(
    conn: &Connection,
    dir: &Path,
    scope_policy: &IndexScopePolicy,
    scopes: &[repository::PermissionScope],
    default_mode: permission_service::PermissionMode,
    allow_once: bool,
    enrichment_queue: Option<&EnrichmentQueue>,
) {
    if !dir.is_dir() || !scope_policy.should_descend_dir(dir) {
        return;
    }

    let parent = canonical(dir);
    let indexed = repository::list_by_parent(conn, &parent).unwrap_or_default();

    let mut on_disk: HashSet<String> = HashSet::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            let is_dir = p.is_dir();
            let canonical_path = canonical(&p);
            if should_skip_path(&p, is_dir, scope_policy) {
                if is_dir {
                    let _ = repository::delete_file_index_subtree(conn, &canonical_path);
                } else {
                    let _ = repository::delete_file_index(conn, &canonical_path);
                }
                continue;
            }
            on_disk.insert(canonical_path);
            process_event_with_default(
                conn,
                &p,
                scope_policy,
                scopes,
                default_mode,
                allow_once,
                enrichment_queue,
            );
        }
    }

    for entry in indexed {
        if !scope_policy.is_path_allowed_str(&entry.path) {
            let _ = repository::delete_file_index(conn, &entry.path);
            continue;
        }
        if !on_disk.contains(&entry.path) {
            let _ = repository::delete_file_index(conn, &entry.path);
        }
    }
}

fn should_skip_reconcile_dir(path: &Path, scope_policy: &IndexScopePolicy) -> bool {
    should_skip_path(path, true, scope_policy)
}

fn reconcile_tree_with_default(
    conn: &Connection,
    root: &Path,
    scope_policy: &IndexScopePolicy,
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
            scope_policy,
            scopes,
            default_mode,
            allow_once,
            enrichment_queue,
        );

        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let child = entry.path();
                if child.is_dir() && !should_skip_reconcile_dir(&child, scope_policy) {
                    stack.push(child);
                }
            }
        }
    }
}

struct EnrichmentWorkerArgs {
    db_path: String,
    scope_policy: IndexScopePolicy,
    last_user_interaction_at: Option<Arc<AtomicI64>>,
    receiver: mpsc::Receiver<String>,
    pending: Arc<Mutex<HashSet<String>>>,
    stop_flag: Arc<AtomicBool>,
    allow_once: bool,
}

fn run_enrichment_worker(args: EnrichmentWorkerArgs) {
    let EnrichmentWorkerArgs {
        db_path,
        scope_policy,
        last_user_interaction_at,
        receiver,
        pending,
        stop_flag,
        allow_once,
    } = args;

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
    let mut embedding_batch = Vec::<embedding_service::EmbeddingDocument>::new();

    while !stop_flag.load(Ordering::Relaxed) {
        if should_yield_for_user(last_user_interaction_at.as_ref()) {
            flush_embedding_batch(&conn, &mut embedding_batch);
            std::thread::sleep(Duration::from_millis(USER_INTERACTION_PAUSE_SLEEP_MS));
            continue;
        }

        let path = match receiver.recv_timeout(Duration::from_millis(250)) {
            Ok(path) => path,
            Err(RecvTimeoutError::Timeout) => {
                flush_embedding_batch(&conn, &mut embedding_batch);
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => break,
        };

        {
            let mut guard = pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.remove(&path);
        }

        let path_buf = Path::new(&path);

        let metadata = match std::fs::metadata(path_buf) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                let _ = repository::delete_file_index(&conn, &path);
                continue;
            }
            Err(_) => continue,
        };

        if should_skip_path(path_buf, metadata.is_dir(), &scope_policy) {
            let _ = repository::delete_file_index(&conn, &path);
            continue;
        }

        if metadata.is_dir() || has_skip_extension(path_buf) {
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

        if let Some(doc) = build_embedding_document(&conn, &entry, allow_once) {
            embedding_batch.push(doc);
            if embedding_batch.len() >= ENRICHMENT_EMBED_BATCH_SIZE {
                flush_embedding_batch(&conn, &mut embedding_batch);
            }
        }
    }

    flush_embedding_batch(&conn, &mut embedding_batch);
}

pub fn start_watching(
    db: Arc<Mutex<Connection>>,
    db_path: String,
    directory: &str,
    allow_once: bool,
    last_user_interaction_at: Option<Arc<AtomicI64>>,
) -> Result<IndexingHandle, AppError> {
    let dir_path = Path::new(directory);
    if !dir_path.is_dir() {
        return Err(AppError::Watcher(format!("not a directory: {directory}")));
    }
    let scope_policy = IndexScopePolicy::build(directory);

    let (enrichment_sender, enrichment_receiver) = mpsc::channel();
    let enrichment_pending = Arc::new(Mutex::new(HashSet::new()));
    let enrichment_queue = EnrichmentQueue {
        sender: enrichment_sender,
        pending: enrichment_pending.clone(),
    };

    let db_clone = db.clone();
    let watcher_enrichment_queue = enrichment_queue.clone();
    let watcher_scope_policy = scope_policy.clone();
    let watcher_user_interaction = last_user_interaction_at.clone();
    let watched_dir = dir_path.to_path_buf();
    let mut debouncer = new_debouncer(
        Duration::from_millis(WATCH_DEBOUNCE_MS),
        move |result: DebounceEventResult| match result {
            Ok(events) => {
                if should_yield_for_user(watcher_user_interaction.as_ref()) {
                    return;
                }
                let Ok(conn) = db_clone.try_lock() else {
                    return;
                };
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
                            if should_skip_reconcile_dir(&event.path, &watcher_scope_policy) {
                                let _ = repository::delete_file_index_subtree(
                                    &conn,
                                    &canonical(event.path.as_path()),
                                );
                                continue;
                            }
                            reconcile_directory_with_default(
                                &conn,
                                &event.path,
                                &watcher_scope_policy,
                                &scopes,
                                default_mode,
                                allow_once,
                                Some(&watcher_enrichment_queue),
                            );
                        } else {
                            if event.path.exists()
                                && should_skip_path(&event.path, false, &watcher_scope_policy)
                            {
                                let _ = repository::delete_file_index(
                                    &conn,
                                    &canonical(event.path.as_path()),
                                );
                                continue;
                            }
                            let path_exists = event.path.exists();
                            process_event_with_default(
                                &conn,
                                &event.path,
                                &watcher_scope_policy,
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
                                        &watcher_scope_policy,
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
        let enrichment_scope_policy = scope_policy.clone();
        let enrichment_user_interaction = last_user_interaction_at.clone();
        let enrichment_pending_for_worker = enrichment_pending;
        Some(std::thread::spawn(move || {
            run_enrichment_worker(EnrichmentWorkerArgs {
                db_path,
                scope_policy: enrichment_scope_policy,
                last_user_interaction_at: enrichment_user_interaction,
                receiver: enrichment_receiver,
                pending: enrichment_pending_for_worker,
                stop_flag: enrichment_stop,
                allow_once,
            });
        }))
    };

    let stop_clone = stop_flag.clone();
    let poll_db = db.clone();
    let poll_scope_policy = scope_policy.clone();
    let poll_dir = watched_dir.clone();
    let poll_enrichment_queue = enrichment_queue.clone();
    let poll_user_interaction = last_user_interaction_at.clone();
    let poller = std::thread::spawn(move || {
        let mut tick = 1u32;
        while !stop_clone.load(Ordering::Relaxed) {
            if should_yield_for_user(poll_user_interaction.as_ref()) {
                std::thread::sleep(Duration::from_millis(USER_INTERACTION_PAUSE_SLEEP_MS));
                continue;
            }

            if let Ok(conn) = poll_db.try_lock() {
                let scopes = repository::get_permission_scopes(&conn).unwrap_or_default();
                let default_mode =
                    permission_service::resolve_default_mode(&conn, PermissionCapability::Indexing)
                        .unwrap_or(permission_service::PermissionMode::Allow);
                if tick.is_multiple_of(POLLER_FULL_RECONCILE_INTERVAL_TICKS) {
                    reconcile_tree_with_default(
                        &conn,
                        &poll_dir,
                        &poll_scope_policy,
                        &scopes,
                        default_mode,
                        allow_once,
                        Some(&poll_enrichment_queue),
                    );
                } else {
                    reconcile_directory_with_default(
                        &conn,
                        &poll_dir,
                        &poll_scope_policy,
                        &scopes,
                        default_mode,
                        allow_once,
                        Some(&poll_enrichment_queue),
                    );
                }

                if tick.is_multiple_of(EMBEDDING_CLEANUP_INTERVAL_TICKS) {
                    if let Err(err) = repository::cleanup_stale_embeddings(&conn) {
                        sentry::capture_message(
                            &format!("stale embedding cleanup failed: {err}"),
                            sentry::Level::Warning,
                        );
                    }
                    if let Err(err) =
                        repository::cleanup_stale_vec_by_age(&conn, VEC_RETENTION_DAYS)
                    {
                        sentry::capture_message(
                            &format!("stale vec age cleanup failed: {err}"),
                            sentry::Level::Warning,
                        );
                    }
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
    // Build artifacts / dependencies
    "node_modules",
    "target",
    "build",
    "dist",
    "out",
    "__pycache__",
    ".venv",
    "venv",
    "env",
    "Pods",
    "vendor",
    "packages",
    "bower_components",
    ".gradle",
    ".mvn",
    ".m2",
    // VCS / IDE
    ".git",
    ".svn",
    ".hg",
    ".idea",
    ".vscode",
    ".vs",
    ".nx",
    // Caches / OS generated
    "CachedData",
    "Cache",
    "Caches",
    "GPUCache",
    "ShaderCache",
    "Code Cache",
    "DerivedData",
    ".Trash",
    ".Trashes",
    ".cache",
    ".next",
    ".nuxt",
    ".svelte-kit",
];

const SKIP_EXTENSIONS: &[&str] = &[
    // Compiled / binary
    "o", "a", "dylib", "so", "dll", "exe", "class", "pyc", "pyo", "wasm", "elf", "msi", "appx",
    // Language specific dependencies / generated
    "gem", "egg", "whl", "nupkg", "jar", "war", "ear", "pyz",
    // Source maps / lockfiles / data dumps
    "map", "lock", "bin", "dat", "db", "sqlite", "sqlite3", "sql", "bson", "rdb", "ibd", "frm",
    // Logs / temp / swaps
    "log", "tmp", "temp", "bak", "swp", "swo", // Icons
    "ico", "icns", "cur", // Archives / disk images
    "zip", "tar", "gz", "bz2", "xz", "7z", "rar", "dmg", "iso", "pkg", "deb", "rpm", "apk", "vdi",
    "vmdk", "ova", "qcow2", // Media
    "mp3", "mp4", "mov", "avi", "mkv", "flac", "wav", "m4a", "aac", "ogg", "m4v", "wmv", "webm",
    "mpg", "mpeg", // Non-OCR images
    "svg", "webp", "gif", "bmp", "tiff", "heic", "raw", "cr2", "nef", "arw", // Fonts
    "ttf", "otf", "woff", "woff2", "eot",
];

const SCAN_PROGRESS_BATCH_SIZE: usize = 50;
const SCAN_PROGRESS_MIN_INTERVAL: Duration = Duration::from_millis(120);
const SCAN_PROGRESS_MAX_SILENCE: Duration = Duration::from_millis(400);

fn should_emit_scan_progress(
    processed: usize,
    total: usize,
    last_reported_processed: usize,
    last_progress_emit: Instant,
) -> bool {
    if total == 0 || processed >= total {
        return true;
    }

    let elapsed = last_progress_emit.elapsed();
    if elapsed >= SCAN_PROGRESS_MAX_SILENCE {
        return true;
    }

    processed.saturating_sub(last_reported_processed) >= SCAN_PROGRESS_BATCH_SIZE
        && elapsed >= SCAN_PROGRESS_MIN_INTERVAL
}

fn should_skip_dir(entry: &walkdir::DirEntry, scope_policy: &IndexScopePolicy) -> bool {
    let path = entry.path();
    should_skip_path(path, entry.file_type().is_dir(), scope_policy)
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
    let scope_policy = IndexScopePolicy::build(directory);

    let walker = || {
        let mut walk = walkdir::WalkDir::new(dir).min_depth(1);
        if let Some(depth) = max_depth {
            walk = walk.max_depth(depth);
        }
        walk.into_iter()
            .filter_entry(|entry| !should_skip_dir(entry, &scope_policy))
            .filter_map(|entry| entry.ok())
    };

    let entries = walker()
        .map(|entry| entry.path().to_path_buf())
        .collect::<Vec<_>>();
    let total = entries.len();
    let scopes = repository::get_permission_scopes(conn).unwrap_or_default();
    let default_mode =
        permission_service::resolve_default_mode(conn, PermissionCapability::Indexing)
            .unwrap_or(permission_service::PermissionMode::Allow);
    let mut processed = 0usize;
    let mut last_progress_emit = Instant::now();
    let mut last_reported_processed = 0usize;

    on_progress(0, total);
    if cancel_flag.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
        return true;
    }

    for path in entries {
        if cancel_flag.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            on_progress(processed, total);
            return true;
        }

        processed += 1;
        process_event_with_default(
            conn,
            path.as_path(),
            &scope_policy,
            &scopes,
            default_mode,
            allow_once,
            enrichment_queue,
        );

        let should_emit_progress = should_emit_scan_progress(
            processed,
            total,
            last_reported_processed,
            last_progress_emit,
        );
        if should_emit_progress {
            on_progress(processed, total);
            last_progress_emit = Instant::now();
            last_reported_processed = processed;
        }
    }

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

pub fn scan_directory_pruned_with_progress_cancel_deferred<F>(
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
    let root = Path::new(directory);
    if !root.is_dir() {
        return false;
    }
    let scope_policy = IndexScopePolicy::build(directory);

    let scopes = repository::get_permission_scopes(conn).unwrap_or_default();
    let default_mode =
        permission_service::resolve_default_mode(conn, PermissionCapability::Indexing)
            .unwrap_or(permission_service::PermissionMode::Allow);

    let mut stack = vec![root.to_path_buf()];
    let mut visited: HashSet<String> = HashSet::new();
    let mut processed_dirs = 0usize;
    let mut discovered_dirs = 1usize;
    let mut last_progress_emit = Instant::now();
    let mut last_reported_processed_dirs = 0usize;

    on_progress(0, discovered_dirs);
    if cancel_flag.load(Ordering::Relaxed) {
        return true;
    }

    while let Some(dir) = stack.pop() {
        if cancel_flag.load(Ordering::Relaxed) {
            on_progress(processed_dirs, discovered_dirs.max(processed_dirs));
            return true;
        }

        let dir_key = canonical(&dir);
        if !visited.insert(dir_key) {
            continue;
        }
        if should_skip_reconcile_dir(&dir, &scope_policy) {
            continue;
        }

        processed_dirs += 1;

        let Some(dir_entry) = file_entry_from_path(&dir) else {
            continue;
        };

        let dir_path = dir_entry.path.clone();
        let dir_modified = dir_entry.modified_at.as_deref().unwrap_or("");
        let should_process_children = repository::needs_reindex(conn, &dir_path, dir_modified);

        if should_process_children {
            process_event_with_default(
                conn,
                &dir,
                &scope_policy,
                &scopes,
                default_mode,
                allow_once,
                Some(enrichment_queue),
            );
        }

        let indexed_children = if should_process_children {
            repository::list_by_parent(conn, &dir_path).unwrap_or_default()
        } else {
            Vec::new()
        };
        let mut on_disk: HashSet<String> = HashSet::new();

        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let child = entry.path();
                let is_dir = child.is_dir();

                // Always recurse into non-skipped subdirectories — parent
                // mtime doesn't reflect changes in nested dirs.
                if is_dir {
                    let canonical_child = canonical(&child);
                    if should_skip_path(&child, true, &scope_policy) {
                        let _ = repository::delete_file_index_subtree(conn, &canonical_child);
                    } else {
                        on_disk.insert(canonical_child);
                        stack.push(child);
                        discovered_dirs += 1;
                    }
                    continue;
                }

                // Directory mtime unchanged — skip file processing
                if !should_process_children {
                    continue;
                }

                let canonical_child = canonical(&child);
                if should_skip_path(&child, false, &scope_policy) {
                    let _ = repository::delete_file_index(conn, &canonical_child);
                    continue;
                }
                on_disk.insert(canonical_child);

                process_event_with_default(
                    conn,
                    &child,
                    &scope_policy,
                    &scopes,
                    default_mode,
                    allow_once,
                    Some(enrichment_queue),
                );

                let total = discovered_dirs.max(processed_dirs);
                if should_emit_scan_progress(
                    processed_dirs,
                    total,
                    last_reported_processed_dirs,
                    last_progress_emit,
                ) {
                    on_progress(processed_dirs, total);
                    last_progress_emit = Instant::now();
                    last_reported_processed_dirs = processed_dirs;
                }
            }
        }

        if should_process_children {
            for entry in indexed_children {
                if !scope_policy.is_path_allowed_str(&entry.path) {
                    let _ = repository::delete_file_index(conn, &entry.path);
                    continue;
                }
                if !on_disk.contains(&entry.path) {
                    let _ = repository::delete_file_index(conn, &entry.path);
                }
            }
        }

        let total = discovered_dirs.max(processed_dirs);
        if should_emit_scan_progress(
            processed_dirs,
            total,
            last_reported_processed_dirs,
            last_progress_emit,
        ) {
            on_progress(processed_dirs, total);
            last_progress_emit = Instant::now();
            last_reported_processed_dirs = processed_dirs;
        }
    }

    on_progress(processed_dirs, processed_dirs);
    false
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
        repository::set_setting(&conn, "embedding_provider", "local").unwrap();
        repository::set_setting(&conn, "embedding_remote_enabled", "0").unwrap();
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
                let conn = db.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                if check(&conn) {
                    return true;
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
        false
    }

    fn canonical(p: &Path) -> String {
        dunce::canonicalize(p)
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
            None,
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
            None,
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
            None,
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
            None,
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
            None,
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

    #[test]
    fn test_pruned_scan_indexes_new_nested_files_on_subsequent_runs() {
        let dir = std::env::temp_dir().join("frogger_test_pruned_scan_nested_updates");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("nested")).unwrap();
        fs::write(dir.join("nested/seed.txt"), "seed").unwrap();

        let conn = test_conn();
        let cancel = AtomicBool::new(false);
        let (sender, _receiver) = mpsc::channel();
        let queue = EnrichmentQueue {
            sender,
            pending: Arc::new(Mutex::new(HashSet::new())),
        };

        let first_cancelled = scan_directory_pruned_with_progress_cancel_deferred(
            &conn,
            dir.to_str().unwrap(),
            false,
            &cancel,
            &queue,
            |_, _| {},
        );
        assert!(!first_cancelled);

        let seed_path = canonical(&dir.join("nested/seed.txt"));
        assert!(repository::get_by_path(&conn, &seed_path)
            .unwrap()
            .is_some());

        fs::write(dir.join("nested/new.txt"), "new").unwrap();

        let second_cancelled = scan_directory_pruned_with_progress_cancel_deferred(
            &conn,
            dir.to_str().unwrap(),
            false,
            &cancel,
            &queue,
            |_, _| {},
        );
        assert!(!second_cancelled);

        let new_path = canonical(&dir.join("nested/new.txt"));
        assert!(repository::get_by_path(&conn, &new_path).unwrap().is_some());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_pruned_scan_skips_unchanged_dir_children() {
        // In-place file edits change the file's mtime but NOT the parent
        // directory's mtime. The pruned scan intentionally skips file
        // processing when the directory mtime is unchanged — deep verify
        // (every 24h) and the file watcher handle this case instead.
        let dir = std::env::temp_dir().join("frogger_test_pruned_scan_offline_edit");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("data.txt");
        fs::write(&file_path, "v1").unwrap();

        let conn = test_conn();
        let cancel = AtomicBool::new(false);
        let (sender, _receiver) = mpsc::channel();
        let queue = EnrichmentQueue {
            sender,
            pending: Arc::new(Mutex::new(HashSet::new())),
        };

        assert!(!scan_directory_pruned_with_progress_cancel_deferred(
            &conn,
            dir.to_str().unwrap(),
            false,
            &cancel,
            &queue,
            |_, _| {},
        ));

        let canonical_path = canonical(&file_path);
        let before = repository::get_by_path(&conn, &canonical_path)
            .unwrap()
            .expect("file should exist after first pruned scan");
        assert_eq!(before.size_bytes, Some(2));

        // Editing file content does not update parent directory mtime.
        fs::write(&file_path, "version two content").unwrap();

        assert!(!scan_directory_pruned_with_progress_cancel_deferred(
            &conn,
            dir.to_str().unwrap(),
            false,
            &cancel,
            &queue,
            |_, _| {},
        ));

        let after = repository::get_by_path(&conn, &canonical_path)
            .unwrap()
            .expect("file should still exist after second pruned scan");
        // File metadata unchanged — pruned scan skipped this directory's children
        assert_eq!(after.size_bytes, Some(2));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_pruned_scan_skips_system_and_hidden_subtrees() {
        let dir = std::env::temp_dir().join("frogger_test_pruned_scan_skips_system_hidden");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("System")).unwrap();
        fs::create_dir_all(dir.join(".cache")).unwrap();
        fs::create_dir_all(dir.join("docs")).unwrap();
        fs::write(dir.join("System/ignore.txt"), "ignore").unwrap();
        fs::write(dir.join(".cache/ignore.txt"), "ignore").unwrap();
        fs::write(dir.join("docs/keep.txt"), "keep").unwrap();

        let conn = test_conn();
        let cancel = AtomicBool::new(false);
        let (sender, _receiver) = mpsc::channel();
        let queue = EnrichmentQueue {
            sender,
            pending: Arc::new(Mutex::new(HashSet::new())),
        };

        assert!(!scan_directory_pruned_with_progress_cancel_deferred(
            &conn,
            dir.to_str().unwrap(),
            false,
            &cancel,
            &queue,
            |_, _| {},
        ));

        let keep_path = canonical(&dir.join("docs/keep.txt"));
        let system_path = canonical(&dir.join("System/ignore.txt"));
        let hidden_path = canonical(&dir.join(".cache/ignore.txt"));

        assert!(repository::get_by_path(&conn, &keep_path)
            .unwrap()
            .is_some());
        assert!(repository::get_by_path(&conn, &system_path)
            .unwrap()
            .is_none());
        assert!(repository::get_by_path(&conn, &hidden_path)
            .unwrap()
            .is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_pruned_scan_skips_top_level_bin_but_keeps_nested_bin() {
        let dir = std::env::temp_dir().join("frogger_test_pruned_scan_nested_bin");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("bin")).unwrap();
        fs::create_dir_all(dir.join("docs/bin")).unwrap();
        fs::write(dir.join("bin/skip.txt"), "skip").unwrap();
        fs::write(dir.join("docs/bin/keep.txt"), "keep").unwrap();

        let conn = test_conn();
        let cancel = AtomicBool::new(false);
        let (sender, _receiver) = mpsc::channel();
        let queue = EnrichmentQueue {
            sender,
            pending: Arc::new(Mutex::new(HashSet::new())),
        };

        assert!(!scan_directory_pruned_with_progress_cancel_deferred(
            &conn,
            dir.to_str().unwrap(),
            false,
            &cancel,
            &queue,
            |_, _| {},
        ));

        let skip_path = canonical(&dir.join("bin/skip.txt"));
        let keep_path = canonical(&dir.join("docs/bin/keep.txt"));

        assert!(repository::get_by_path(&conn, &skip_path)
            .unwrap()
            .is_none());
        assert!(repository::get_by_path(&conn, &keep_path)
            .unwrap()
            .is_some());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_watcher_ignores_hidden_file_events() {
        let dir = std::env::temp_dir().join("frogger_test_watcher_hidden_file_skip");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let conn = test_conn();
        let db = Arc::new(Mutex::new(conn));
        let handle = start_watching(
            db.clone(),
            ":memory:".to_string(),
            dir.to_str().unwrap(),
            false,
            None,
        )
        .unwrap();

        let visible_file = dir.join("visible.txt");
        let hidden_file = dir.join(".hidden.txt");
        fs::write(&visible_file, "hello").unwrap();
        fs::write(&hidden_file, "secret").unwrap();

        let visible_path = canonical(&visible_file);
        let hidden_path = canonical(&hidden_file);
        let visible_found = poll_until(&db, 5000, |conn| {
            repository::get_by_path(conn, &visible_path)
                .unwrap()
                .is_some()
        });
        let hidden_found = poll_until(&db, 1500, |conn| {
            repository::get_by_path(conn, &hidden_path)
                .unwrap()
                .is_some()
        });

        stop_watching(handle);
        let _ = fs::remove_dir_all(&dir);

        assert!(visible_found, "visible file should be indexed");
        assert!(!hidden_found, "hidden file should be ignored");
    }

    #[test]
    fn test_scan_progress_emits_initial_and_final_updates() {
        let dir = std::env::temp_dir().join("frogger_test_scan_progress_cadence");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        for idx in 0..25 {
            fs::write(
                dir.join(format!("file-{idx}.txt")),
                format!("content-{idx}"),
            )
            .unwrap();
        }

        let conn = test_conn();
        let cancel = AtomicBool::new(false);
        let progress = Mutex::new(Vec::<(usize, usize)>::new());

        let cancelled = scan_directory_internal(
            &conn,
            dir.to_str().unwrap(),
            false,
            None,
            Some(&cancel),
            None,
            |processed, total| {
                progress
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push((processed, total));
            },
        );

        assert!(!cancelled);
        let updates = progress
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        assert!(
            updates.len() >= 2,
            "expected at least initial and final progress updates"
        );
        assert_eq!(updates.first().copied(), Some((0, 25)));
        assert_eq!(updates.last().copied(), Some((25, 25)));

        let _ = fs::remove_dir_all(&dir);
    }
}
