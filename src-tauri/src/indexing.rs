use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use chrono::{DateTime, SecondsFormat, Utc};
use ignore::WalkBuilder;
use rusqlite::{params, Connection};
use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::models::{EventNames, IndexingState, IndexingStatus};
use crate::persistence;

const METADATA_INDEX_ID: &str = "metadata";
const DEFAULT_BATCH_SIZE: usize = 1_000;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(750);

static SCHEDULED_DATABASES: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexingSummary {
    pub indexed_item_count: u64,
    pub dirs_visited: u64,
    pub files_visited: u64,
    pub metadata_errors: u64,
    pub pruned_item_count: u64,
    pub elapsed_ms: u128,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct IndexingCounters {
    indexed_item_count: u64,
    dirs_visited: u64,
    files_visited: u64,
    metadata_errors: u64,
    pruned_item_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MetadataIndexRow {
    path: String,
    parent_path: String,
    name: String,
    display_name: String,
    kind: String,
    is_dir: bool,
    size: Option<u64>,
    modified_at: Option<String>,
    created_at: Option<String>,
    hidden: bool,
    extension: Option<String>,
    search_text: String,
}

/// Returns the default local-only metadata indexing roots for the current user.
///
/// The home directory is the primary root. Cloud-storage roots that live inside
/// macOS Library are added separately because the home crawl intentionally skips
/// Library as an application/system-internal tree.
pub fn default_index_roots(home_dir: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    push_existing_dir(&mut roots, home_dir.to_path_buf());

    let icloud_drive = home_dir
        .join("Library")
        .join("Mobile Documents")
        .join("com~apple~CloudDocs");
    push_existing_dir(&mut roots, icloud_drive);

    let cloud_storage = home_dir.join("Library").join("CloudStorage");
    if let Ok(entries) = std::fs::read_dir(cloud_storage) {
        for entry in entries.flatten() {
            push_existing_dir(&mut roots, entry.path());
        }
    }

    roots
}

fn push_existing_dir(roots: &mut Vec<PathBuf>, path: PathBuf) {
    if !path.is_dir() || roots.iter().any(|existing| existing == &path) {
        return;
    }

    roots.push(path);
}

/// Starts one metadata indexing/reconciliation run for this app process.
///
/// Multiple webviews may call bootstrap; this guard prevents duplicate
/// filesystem crawls competing with each other. Failed runs are removed from the
/// guard so a later bootstrap can retry.
pub fn ensure_metadata_index_started(
    app: &AppHandle,
    database_path: PathBuf,
    home_dir: PathBuf,
) -> Result<bool> {
    let roots = default_index_roots(&home_dir);
    if roots.is_empty() {
        return Ok(false);
    }

    let scheduled = SCHEDULED_DATABASES.get_or_init(|| Mutex::new(HashSet::new()));
    {
        let mut scheduled = scheduled
            .lock()
            .expect("metadata index scheduled-database guard should not be poisoned");
        if !scheduled.insert(database_path.clone()) {
            return Ok(false);
        }
    }

    // Mark the run as started before bootstrap returns so the frontend sees a
    // truthful indexing state even if it misses the first async event.
    let mut conn = persistence::open_database(&database_path)?;
    let initial_state = mark_run_started(&mut conn, &roots)?;
    emit_indexing_state(app, &initial_state);
    drop(conn);

    let app_handle = app.clone();
    let thread_database_path = database_path.clone();
    let thread_roots = roots.clone();
    let spawn_result = thread::Builder::new()
        .name("frogger-metadata-index".to_string())
        .spawn(move || {
            let result = run_metadata_index(&thread_database_path, thread_roots, |state| {
                emit_indexing_state(&app_handle, &state);
            });

            if let Err(error) = result {
                if let Ok(mut conn) = persistence::open_database(&thread_database_path) {
                    let had_initial_index = has_initial_index(&conn).unwrap_or(false);
                    let state = mark_run_failed(&mut conn, had_initial_index, &error.to_string())
                        .unwrap_or_else(|_| IndexingState {
                            status: IndexingStatus::Failed,
                            has_initial_index: had_initial_index,
                            indexed_item_count: 0,
                            message: Some(error.to_string()),
                        });
                    emit_indexing_state(&app_handle, &state);
                }

                let scheduled = SCHEDULED_DATABASES.get_or_init(|| Mutex::new(HashSet::new()));
                if let Ok(mut scheduled) = scheduled.lock() {
                    scheduled.remove(&thread_database_path);
                }
            }
        });

    if let Err(error) = spawn_result {
        let scheduled = SCHEDULED_DATABASES.get_or_init(|| Mutex::new(HashSet::new()));
        if let Ok(mut scheduled) = scheduled.lock() {
            scheduled.remove(&database_path);
        }

        let had_initial_index = has_initial_index(&persistence::open_database(&database_path)?)?;
        let mut conn = persistence::open_database(&database_path)?;
        mark_run_failed(&mut conn, had_initial_index, &error.to_string())?;
        return Err(error.into());
    }

    Ok(true)
}

pub fn run_metadata_index<F>(
    database_path: &Path,
    roots: Vec<PathBuf>,
    mut on_progress: F,
) -> Result<IndexingSummary>
where
    F: FnMut(IndexingState),
{
    let started = Instant::now();
    let mut conn = persistence::open_database(database_path)?;
    let run_started_at = now_rfc3339();
    let mut counters = IndexingCounters::default();
    let mut batch = Vec::with_capacity(DEFAULT_BATCH_SIZE);
    let mut last_progress = Instant::now();

    let started_state = mark_run_started(&mut conn, &roots)?;
    let initial_build = !started_state.has_initial_index;
    on_progress(started_state);

    for root in roots.iter().filter(|root| root.is_dir()) {
        let mut builder = WalkBuilder::new(root);
        builder
            .follow_links(false)
            .hidden(false)
            .ignore(false)
            .git_ignore(false)
            .git_global(false)
            .git_exclude(false)
            .parents(false);

        let home_for_exclusions = roots.first().cloned();
        let walk = builder
            .filter_entry(move |entry| {
                entry.depth() == 0
                    || !is_default_excluded_path(
                        entry.path(),
                        entry
                            .file_type()
                            .map(|file_type| file_type.is_dir())
                            .unwrap_or(false),
                        home_for_exclusions.as_deref(),
                    )
            })
            .build();

        for entry_result in walk {
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(_error) => {
                    counters.metadata_errors += 1;
                    continue;
                }
            };

            let is_dir = entry
                .file_type()
                .map(|file_type| file_type.is_dir())
                .unwrap_or(false);
            if is_dir {
                counters.dirs_visited += 1;
            } else {
                counters.files_visited += 1;
            }

            match metadata_row_from_path(entry.path()) {
                Ok(Some(row)) => batch.push(row),
                Ok(None) => {}
                Err(_error) => counters.metadata_errors += 1,
            }

            if batch.len() >= DEFAULT_BATCH_SIZE {
                flush_batch(&mut conn, &mut batch, &run_started_at)?;
                counters.indexed_item_count = count_indexed_items_from_table(&conn)?;
                let status = if initial_build {
                    IndexingStatus::InitialBuild
                } else {
                    IndexingStatus::Reconciling
                };
                let state =
                    mark_run_progress(&mut conn, status, true, &roots, &counters, "Indexing…")?;
                on_progress(state);
                last_progress = Instant::now();
            } else if last_progress.elapsed() >= PROGRESS_INTERVAL {
                let has_initial = has_initial_index(&conn)? || counters.indexed_item_count > 0;
                let state = progress_state(
                    if initial_build {
                        IndexingStatus::InitialBuild
                    } else if has_initial {
                        IndexingStatus::Reconciling
                    } else {
                        IndexingStatus::InitialBuild
                    },
                    has_initial,
                    counters.indexed_item_count,
                    Some("Indexing…".to_string()),
                );
                on_progress(state);
                last_progress = Instant::now();
            }
        }
    }

    if !batch.is_empty() {
        flush_batch(&mut conn, &mut batch, &run_started_at)?;
    }

    counters.pruned_item_count = prune_stale_rows(&conn, &roots, &run_started_at)?;
    counters.indexed_item_count = count_indexed_items_from_table(&conn)?;
    let completed_state = mark_run_completed(&mut conn, &roots, &counters)?;
    on_progress(completed_state);

    Ok(IndexingSummary {
        indexed_item_count: counters.indexed_item_count,
        dirs_visited: counters.dirs_visited,
        files_visited: counters.files_visited,
        metadata_errors: counters.metadata_errors,
        pruned_item_count: counters.pruned_item_count,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

pub(crate) fn is_default_excluded_path(path: &Path, is_dir: bool, home_dir: Option<&Path>) -> bool {
    if !is_dir {
        return false;
    }

    if home_dir
        .map(|home| {
            path == home.join("Library")
                || path == home.join("Applications")
                || path == home.join(".Trash")
        })
        .unwrap_or(false)
    {
        return true;
    }

    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();

    if lower.starts_with('.') {
        return true;
    }

    matches!(
        lower.as_str(),
        "node_modules"
            | "target"
            | "dist"
            | "build"
            | "coverage"
            | "__pycache__"
            | "vendor"
            | "tmp"
            | "temp"
            | "cache"
            | "caches"
            | "appdata"
            | "system volume information"
            | "$recycle.bin"
            | "venv"
            | "env"
    )
}

fn metadata_row_from_path(path: &Path) -> Result<Option<MetadataIndexRow>> {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .map(ToString::to_string)
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    if name.is_empty() {
        return Ok(None);
    }

    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("failed to read metadata for {}", path.display()))?;
    let file_type = metadata.file_type();
    let is_symlink = file_type.is_symlink();
    let is_dir = file_type.is_dir();
    let hidden = name.starts_with('.');
    let extension = (!is_dir)
        .then(|| {
            path.extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_ascii_lowercase())
        })
        .flatten();
    let parent_path = path
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned())
        .unwrap_or_default();
    let kind = if is_symlink {
        "Alias".to_string()
    } else {
        kind_for(is_dir, extension.as_deref()).to_string()
    };
    let display_name = name.clone();
    let path_string = path.to_string_lossy().into_owned();
    let search_text = format!(
        "{} {} {} {} {}",
        name,
        display_name,
        parent_path,
        kind,
        extension.as_deref().unwrap_or_default()
    )
    .to_ascii_lowercase();

    Ok(Some(MetadataIndexRow {
        path: path_string,
        parent_path,
        name,
        display_name,
        kind,
        is_dir,
        size: (!is_dir).then_some(metadata.len()),
        modified_at: metadata.modified().ok().map(system_time_to_rfc3339),
        created_at: metadata.created().ok().map(system_time_to_rfc3339),
        hidden,
        extension,
        search_text,
    }))
}

fn flush_batch(
    conn: &mut Connection,
    batch: &mut Vec<MetadataIndexRow>,
    indexed_at: &str,
) -> Result<()> {
    if batch.is_empty() {
        return Ok(());
    }

    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO metadata_index (
                path, parent_path, name, display_name, kind, is_dir, size,
                modified_at, created_at, indexed_at, hidden, extension, search_text
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(path) DO UPDATE SET
                parent_path = excluded.parent_path,
                name = excluded.name,
                display_name = excluded.display_name,
                kind = excluded.kind,
                is_dir = excluded.is_dir,
                size = excluded.size,
                modified_at = excluded.modified_at,
                created_at = excluded.created_at,
                indexed_at = excluded.indexed_at,
                hidden = excluded.hidden,
                extension = excluded.extension,
                search_text = excluded.search_text",
        )?;

        for row in batch.iter() {
            stmt.execute(params![
                &row.path,
                &row.parent_path,
                &row.name,
                &row.display_name,
                &row.kind,
                bool_to_i64(row.is_dir),
                row.size.map(|value| value as i64),
                row.modified_at.as_deref(),
                row.created_at.as_deref(),
                indexed_at,
                bool_to_i64(row.hidden),
                row.extension.as_deref(),
                &row.search_text,
            ])?;
        }
    }
    tx.commit()?;
    batch.clear();
    Ok(())
}

fn mark_run_started(conn: &mut Connection, roots: &[PathBuf]) -> Result<IndexingState> {
    let has_initial = has_initial_index(conn)?;
    let status = if has_initial {
        IndexingStatus::Reconciling
    } else {
        IndexingStatus::InitialBuild
    };
    let count = count_indexed_items_from_checkpoint_or_table(conn)?;
    let checkpoint = checkpoint_json(
        roots,
        &IndexingCounters {
            indexed_item_count: count,
            ..IndexingCounters::default()
        },
        "Indexing…",
    );
    let now = now_rfc3339();

    conn.execute(
        "UPDATE index_state SET
            status = ?1,
            has_initial_index = ?2,
            started_at = ?3,
            completed_at = NULL,
            checkpoint_json = ?4,
            error_json = NULL,
            updated_at = ?3
         WHERE id = ?5",
        params![
            status_to_db(&status),
            bool_to_i64(has_initial),
            now,
            checkpoint,
            METADATA_INDEX_ID,
        ],
    )?;

    Ok(progress_state(
        status,
        has_initial,
        count,
        Some("Indexing…".to_string()),
    ))
}

fn mark_run_progress(
    conn: &mut Connection,
    status: IndexingStatus,
    has_initial: bool,
    roots: &[PathBuf],
    counters: &IndexingCounters,
    message: &str,
) -> Result<IndexingState> {
    let checkpoint = checkpoint_json(roots, counters, message);
    let now = now_rfc3339();
    conn.execute(
        "UPDATE index_state SET
            status = ?1,
            has_initial_index = ?2,
            checkpoint_json = ?3,
            updated_at = ?4
         WHERE id = ?5",
        params![
            status_to_db(&status),
            bool_to_i64(has_initial),
            checkpoint,
            now,
            METADATA_INDEX_ID,
        ],
    )?;

    Ok(progress_state(
        status,
        has_initial,
        counters.indexed_item_count,
        Some(message.to_string()),
    ))
}

fn mark_run_completed(
    conn: &mut Connection,
    roots: &[PathBuf],
    counters: &IndexingCounters,
) -> Result<IndexingState> {
    let checkpoint = checkpoint_json(roots, counters, "Index ready");
    let now = now_rfc3339();
    conn.execute(
        "UPDATE index_state SET
            status = 'ready',
            has_initial_index = 1,
            completed_at = ?1,
            checkpoint_json = ?2,
            error_json = NULL,
            updated_at = ?1
         WHERE id = ?3",
        params![now, checkpoint, METADATA_INDEX_ID],
    )?;

    Ok(progress_state(
        IndexingStatus::Ready,
        true,
        counters.indexed_item_count,
        Some("Index ready".to_string()),
    ))
}

fn mark_run_failed(
    conn: &mut Connection,
    had_initial_index: bool,
    message: &str,
) -> Result<IndexingState> {
    let error_json = json!({ "message": message }).to_string();
    let now = now_rfc3339();
    let count = count_indexed_items_from_checkpoint_or_table(conn).unwrap_or(0);
    conn.execute(
        "UPDATE index_state SET
            status = 'failed',
            has_initial_index = ?1,
            error_json = ?2,
            updated_at = ?3
         WHERE id = ?4",
        params![
            bool_to_i64(had_initial_index),
            error_json,
            now,
            METADATA_INDEX_ID,
        ],
    )?;

    Ok(progress_state(
        IndexingStatus::Failed,
        had_initial_index,
        count,
        Some(message.to_string()),
    ))
}

fn checkpoint_json(roots: &[PathBuf], counters: &IndexingCounters, message: &str) -> String {
    json!({
        "indexedItemCount": counters.indexed_item_count,
        "dirsVisited": counters.dirs_visited,
        "filesVisited": counters.files_visited,
        "metadataErrors": counters.metadata_errors,
        "prunedItemCount": counters.pruned_item_count,
        "roots": roots.iter().map(|root| root.to_string_lossy().into_owned()).collect::<Vec<_>>(),
        "message": message,
    })
    .to_string()
}

fn prune_stale_rows(conn: &Connection, roots: &[PathBuf], run_started_at: &str) -> Result<u64> {
    let mut pruned = 0_u64;
    for root in roots {
        let root_string = root.to_string_lossy().into_owned();
        let pattern = child_like_pattern(&root_string);
        let affected = conn.execute(
            "DELETE FROM metadata_index
             WHERE (path = ?1 OR path LIKE ?2 ESCAPE '\\') AND indexed_at < ?3",
            params![root_string, pattern, run_started_at],
        )?;
        pruned += affected as u64;
    }

    Ok(pruned)
}

fn child_like_pattern(root: &str) -> String {
    let separator = std::path::MAIN_SEPARATOR;
    let mut pattern = escape_like(root);
    if !pattern.ends_with(separator) {
        pattern.push(separator);
    }
    pattern.push('%');
    pattern
}

fn escape_like(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| match character {
            '%' | '_' | '\\' => ['\\', character],
            _ => ['\0', character],
        })
        .filter(|character| *character != '\0')
        .collect()
}

fn has_initial_index(conn: &Connection) -> Result<bool> {
    let has_initial_index: i64 = conn.query_row(
        "SELECT has_initial_index FROM index_state WHERE id = ?1",
        [METADATA_INDEX_ID],
        |row| row.get(0),
    )?;
    Ok(has_initial_index == 1)
}

fn count_indexed_items_from_table(conn: &Connection) -> Result<u64> {
    conn.query_row("SELECT COUNT(*) FROM metadata_index", [], |row| row.get(0))
        .context("failed to count indexed metadata rows")
}

fn count_indexed_items_from_checkpoint_or_table(conn: &Connection) -> Result<u64> {
    let checkpoint_json: String = conn.query_row(
        "SELECT checkpoint_json FROM index_state WHERE id = ?1",
        [METADATA_INDEX_ID],
        |row| row.get(0),
    )?;

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&checkpoint_json) {
        if let Some(count) = value
            .get("indexedItemCount")
            .and_then(|count| count.as_u64())
        {
            return Ok(count);
        }
    }

    conn.query_row("SELECT COUNT(*) FROM metadata_index", [], |row| row.get(0))
        .context("failed to count indexed metadata rows")
}

fn progress_state(
    status: IndexingStatus,
    has_initial_index: bool,
    indexed_item_count: u64,
    message: Option<String>,
) -> IndexingState {
    IndexingState {
        status,
        has_initial_index,
        indexed_item_count,
        message,
    }
}

fn emit_indexing_state(app: &AppHandle, state: &IndexingState) {
    let _ = app.emit(&EventNames::default().indexing_progress, state.clone());
}

fn status_to_db(status: &IndexingStatus) -> &'static str {
    match status {
        IndexingStatus::NotStarted => "not_started",
        IndexingStatus::InitialBuild => "initial_build",
        IndexingStatus::Reconciling => "reconciling",
        IndexingStatus::Ready => "ready",
        IndexingStatus::Failed => "failed",
    }
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn system_time_to_rfc3339(value: SystemTime) -> String {
    DateTime::<Utc>::from(value).to_rfc3339()
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Nanos, true)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileCategory {
    Application,
    Archive,
    Audio,
    Document,
    Image,
    Markdown,
    Pdf,
    SourceCode,
    Spreadsheet,
    Text,
    Video,
    WordDocument,
}

fn file_category(extension: Option<&str>) -> FileCategory {
    match extension.map(|value| value.to_ascii_lowercase()).as_deref() {
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "heic" | "svg") => FileCategory::Image,
        Some("mov" | "mp4" | "m4v" | "avi" | "mkv" | "webm") => FileCategory::Video,
        Some("mp3" | "wav" | "aac" | "flac" | "m4a" | "ogg") => FileCategory::Audio,
        Some("pdf") => FileCategory::Pdf,
        Some("xls" | "xlsx" | "xlsm" | "xlsb" | "csv" | "tsv" | "ods" | "numbers") => {
            FileCategory::Spreadsheet
        }
        Some("doc" | "docx" | "odt" | "pages") => FileCategory::WordDocument,
        Some("md" | "markdown") => FileCategory::Markdown,
        Some("txt" | "rtf" | "log") => FileCategory::Text,
        Some("zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar") => FileCategory::Archive,
        Some("app" | "exe" | "dmg" | "pkg") => FileCategory::Application,
        Some("rs" | "ts" | "tsx" | "js" | "jsx" | "json" | "html" | "css" | "scss") => {
            FileCategory::SourceCode
        }
        Some(_) | None => FileCategory::Document,
    }
}

fn kind_for(is_dir: bool, extension: Option<&str>) -> &'static str {
    if is_dir {
        return "Folder";
    }

    match file_category(extension) {
        FileCategory::Application => "Application",
        FileCategory::Archive => "Archive",
        FileCategory::Audio => "Audio",
        FileCategory::Document => "Document",
        FileCategory::Image => "Image",
        FileCategory::Markdown => "Markdown Document",
        FileCategory::Pdf => "PDF Document",
        FileCategory::SourceCode => "Source Code",
        FileCategory::Spreadsheet => "Spreadsheet",
        FileCategory::Text => "Text Document",
        FileCategory::Video => "Video",
        FileCategory::WordDocument => "Word Document",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use uuid::Uuid;

    #[test]
    fn default_exclusions_skip_noisy_indexing_dirs() {
        let temp = tempdir().expect("tempdir should exist");
        let home = temp.path();

        assert!(is_default_excluded_path(
            &home.join("node_modules"),
            true,
            Some(home)
        ));
        assert!(is_default_excluded_path(
            &home.join(".git"),
            true,
            Some(home)
        ));
        assert!(is_default_excluded_path(
            &home.join("Library"),
            true,
            Some(home)
        ));
        assert!(!is_default_excluded_path(
            &home.join("Documents"),
            true,
            Some(home)
        ));
        assert!(!is_default_excluded_path(
            &home.join("Cargo.lock"),
            false,
            Some(home)
        ));
    }

    #[test]
    fn metadata_indexer_prunes_excluded_dirs_and_batches_rows() {
        let temp = tempdir().expect("tempdir should exist");
        let root = temp.path().join("home");
        std::fs::create_dir_all(root.join("src")).expect("src dir should be created");
        std::fs::create_dir_all(root.join("node_modules/pkg"))
            .expect("node_modules should be created");
        std::fs::create_dir_all(root.join("target/debug")).expect("target should be created");
        std::fs::write(root.join("README.md"), "hello").expect("readme should write");
        std::fs::write(root.join("src/main.rs"), "fn main() {}").expect("source should write");
        std::fs::write(
            root.join("node_modules/pkg/index.js"),
            "console.log('skip')",
        )
        .expect("dependency file should write");
        std::fs::write(root.join("target/debug/app"), "skip").expect("target file should write");

        let database_path = temp
            .path()
            .join(format!("frogger-index-{}.sqlite3", Uuid::new_v4()));
        let summary = run_metadata_index(&database_path, vec![root.clone()], |_| {})
            .expect("metadata index should run");

        assert!(summary.indexed_item_count >= 3);
        assert_eq!(summary.metadata_errors, 0);

        let conn = persistence::open_database(&database_path).expect("database should open");
        let indexed_paths = indexed_paths(&conn);
        assert!(indexed_paths.contains(&root.join("README.md").to_string_lossy().into_owned()));
        assert!(indexed_paths.contains(&root.join("src/main.rs").to_string_lossy().into_owned()));
        assert!(!indexed_paths
            .iter()
            .any(|path| path.contains("node_modules") || path.contains("target/debug")));

        let (status, has_initial_index): (String, i64) = conn
            .query_row(
                "SELECT status, has_initial_index FROM index_state WHERE id = 'metadata'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("state should load");
        assert_eq!(status, "ready");
        assert_eq!(has_initial_index, 1);
    }

    #[test]
    fn metadata_reconciliation_prunes_deleted_paths() {
        let temp = tempdir().expect("tempdir should exist");
        let root = temp.path().join("home");
        std::fs::create_dir_all(&root).expect("root should be created");
        let stale = root.join("stale.txt");
        let fresh = root.join("fresh.txt");
        std::fs::write(&stale, "stale").expect("stale file should write");
        std::fs::write(&fresh, "fresh").expect("fresh file should write");

        let database_path = temp
            .path()
            .join(format!("frogger-reconcile-{}.sqlite3", Uuid::new_v4()));
        run_metadata_index(&database_path, vec![root.clone()], |_| {})
            .expect("first index should run");
        std::fs::remove_file(&stale).expect("stale file should be removed");
        let summary = run_metadata_index(&database_path, vec![root.clone()], |_| {})
            .expect("second index should run");

        assert!(summary.pruned_item_count >= 1);
        let conn = persistence::open_database(&database_path).expect("database should open");
        let indexed_paths = indexed_paths(&conn);
        assert!(!indexed_paths.contains(&stale.to_string_lossy().into_owned()));
        assert!(indexed_paths.contains(&fresh.to_string_lossy().into_owned()));
    }

    fn indexed_paths(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT path FROM metadata_index ORDER BY path")
            .expect("path query should prepare");
        stmt.query_map([], |row| row.get::<_, String>(0))
            .expect("path query should run")
            .collect::<Result<Vec<_>, _>>()
            .expect("paths should collect")
    }
}
