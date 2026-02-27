use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{command, AppHandle, Emitter, State};

use crate::data::{migrations, repository};
use crate::error::AppError;
use crate::services::indexing_service;
use crate::services::permission_service::{self, PermissionCapability};
use crate::state::{AppState, IndexingProgressState};

const INDEXING_LAST_DEEP_VERIFY_AT_KEY: &str = "indexing_last_deep_verify_at";
const INDEXING_DEEP_VERIFY_INTERVAL_SECS: i64 = 24 * 60 * 60;
const INDEX_SCOPE_POLICY_VERSION_KEY: &str = "index_scope_policy_version";
const INDEX_SCOPE_POLICY_MAINTENANCE_IN_PROGRESS_KEY: &str =
    "index_scope_policy_maintenance_in_progress";
const INDEX_SCOPE_POLICY_MAINTENANCE_STARTED_AT_KEY: &str =
    "index_scope_policy_maintenance_started_at";
const INDEX_SCOPE_POLICY_VERSION: i64 = 2;
const USER_INTERACTION_PAUSE_MS: i64 = 1500;
const USER_INTERACTION_PAUSE_SLEEP_MS: u64 = 120;

fn emit_indexing_progress(
    app: &AppHandle,
    indexing_status: &Arc<Mutex<IndexingProgressState>>,
    processed: usize,
    total: usize,
    status: &str,
) {
    let payload = IndexingProgressState {
        processed,
        total,
        status: status.to_string(),
    };

    if let Ok(mut guard) = indexing_status.lock() {
        *guard = payload.clone();
    }

    let _ = app.emit("indexing-progress", &payload);
}

fn progress_status(processed: usize, total: usize) -> &'static str {
    if total == 0 || processed >= total {
        "done"
    } else {
        "active"
    }
}

fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn should_pause_for_user(last_user_interaction_at: &Arc<AtomicI64>) -> bool {
    let last = last_user_interaction_at.load(Ordering::Relaxed);
    last > 0 && now_millis().saturating_sub(last) < USER_INTERACTION_PAUSE_MS
}

fn wait_for_user_idle(
    last_user_interaction_at: &Arc<AtomicI64>,
    app: &AppHandle,
    indexing_status: &Arc<Mutex<IndexingProgressState>>,
) {
    let mut emitted_paused = false;
    while should_pause_for_user(last_user_interaction_at) {
        if !emitted_paused {
            emit_indexing_progress(app, indexing_status, 0, 0, "paused_for_user");
            emitted_paused = true;
        }
        std::thread::sleep(Duration::from_millis(USER_INTERACTION_PAUSE_SLEEP_MS));
    }
}

fn should_run_deep_verify(conn: &rusqlite::Connection) -> bool {
    let now = chrono::Utc::now().timestamp();
    let Some(raw) = repository::get_setting(conn, INDEXING_LAST_DEEP_VERIFY_AT_KEY).unwrap_or(None)
    else {
        return true;
    };

    let Ok(last_run) = raw.parse::<i64>() else {
        return true;
    };

    now.saturating_sub(last_run) >= INDEXING_DEEP_VERIFY_INTERVAL_SECS
}

fn mark_deep_verify_now(conn: &rusqlite::Connection) -> Result<(), AppError> {
    let now = chrono::Utc::now().timestamp().to_string();
    repository::set_setting(conn, INDEXING_LAST_DEEP_VERIFY_AT_KEY, &now)
}

fn current_index_scope_policy_version(conn: &rusqlite::Connection) -> i64 {
    repository::get_setting(conn, INDEX_SCOPE_POLICY_VERSION_KEY)
        .ok()
        .flatten()
        .and_then(|raw| raw.parse::<i64>().ok())
        .unwrap_or(0)
}

fn maybe_apply_index_scope_policy(
    conn: &rusqlite::Connection,
    directory: &str,
) -> Result<usize, AppError> {
    if current_index_scope_policy_version(conn) >= INDEX_SCOPE_POLICY_VERSION {
        return Ok(0);
    }

    let allowed_roots = indexing_service::resolve_allowed_roots_for_directory(directory);
    let blocked_roots = indexing_service::resolve_blocked_roots_for_directory(directory);
    let removed = repository::prune_index_outside_scope(conn, &allowed_roots, &blocked_roots)?;
    repository::set_setting(
        conn,
        INDEX_SCOPE_POLICY_VERSION_KEY,
        &INDEX_SCOPE_POLICY_VERSION.to_string(),
    )?;
    Ok(removed)
}

fn mark_index_scope_maintenance_started(conn: &rusqlite::Connection) -> Result<(), AppError> {
    repository::set_setting(conn, INDEX_SCOPE_POLICY_MAINTENANCE_IN_PROGRESS_KEY, "1")?;
    repository::set_setting(
        conn,
        INDEX_SCOPE_POLICY_MAINTENANCE_STARTED_AT_KEY,
        &chrono::Utc::now().timestamp().to_string(),
    )?;
    Ok(())
}

fn clear_index_scope_maintenance_state(conn: &rusqlite::Connection) {
    let _ = repository::set_setting(conn, INDEX_SCOPE_POLICY_MAINTENANCE_IN_PROGRESS_KEY, "0");
    let _ = repository::set_setting(conn, INDEX_SCOPE_POLICY_MAINTENANCE_STARTED_AT_KEY, "0");
}

fn run_index_scope_policy_maintenance(
    conn: &rusqlite::Connection,
    directory: &str,
    app: &AppHandle,
    indexing_status: &Arc<Mutex<IndexingProgressState>>,
) -> Result<usize, AppError> {
    if current_index_scope_policy_version(conn) >= INDEX_SCOPE_POLICY_VERSION {
        return Ok(0);
    }

    emit_indexing_progress(app, indexing_status, 0, 0, "maintenance");
    mark_index_scope_maintenance_started(conn)?;
    let result = maybe_apply_index_scope_policy(conn, directory);
    clear_index_scope_maintenance_state(conn);
    result
}

#[command]
pub fn notify_user_interaction(state: State<'_, AppState>) -> Result<(), AppError> {
    state
        .last_user_interaction_at
        .store(now_millis(), Ordering::Relaxed);
    Ok(())
}

#[command]
pub fn start_indexing(
    directory: String,
    allow_once: Option<bool>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let allow_once = allow_once.unwrap_or(false);

    // Hold watcher_handle lock across entire setup to prevent race where two
    // concurrent calls both pass the initial check and create duplicate watchers.
    let mut handle_guard = state.watcher_handle.lock().unwrap();
    if handle_guard.is_some() {
        return Ok(());
    }

    {
        let conn = state
            .db
            .lock()
            .map_err(|e| AppError::General(e.to_string()))?;
        permission_service::enforce_cached(
            &conn,
            &state,
            &directory,
            PermissionCapability::Indexing,
            allow_once,
        )?;
    }
    state
        .last_user_interaction_at
        .store(now_millis(), Ordering::Relaxed);
    let user_interaction_tracker = state.last_user_interaction_at.clone();

    let conn =
        rusqlite::Connection::open(&state.db_path).map_err(|e| AppError::Watcher(e.to_string()))?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|e| AppError::Watcher(e.to_string()))?;
    migrations::run_migrations(&conn)?;
    let db = Arc::new(Mutex::new(conn));
    let indexing_status = state.indexing_status.clone();
    let mut watcher = indexing_service::start_watching(
        db,
        state.db_path.to_string_lossy().to_string(),
        &directory,
        allow_once,
        Some(user_interaction_tracker.clone()),
    )?;
    emit_indexing_progress(&app, &indexing_status, 0, 0, "starting");

    let stop_flag = watcher.stop_flag();
    let enrichment_queue = watcher.enrichment_queue();
    let db_path_for_scan = state.db_path.clone();
    let directory_for_scan = directory.clone();
    let app_for_scan = app.clone();
    let indexing_status_for_scan = indexing_status.clone();
    let user_interaction_for_scan = user_interaction_tracker.clone();
    let bootstrap_worker = std::thread::spawn(move || {
        let emitted_progress = AtomicBool::new(false);
        let cancelled = match rusqlite::Connection::open(&db_path_for_scan) {
            Ok(conn) => {
                let _ = conn.busy_timeout(std::time::Duration::from_secs(5));
                wait_for_user_idle(
                    &user_interaction_for_scan,
                    &app_for_scan,
                    &indexing_status_for_scan,
                );
                if let Err(err) = run_index_scope_policy_maintenance(
                    &conn,
                    &directory_for_scan,
                    &app_for_scan,
                    &indexing_status_for_scan,
                ) {
                    sentry::capture_message(
                        &format!("index scope maintenance failed: {err}"),
                        sentry::Level::Warning,
                    );
                }

                let run_deep_verify = should_run_deep_verify(&conn);
                let cancelled = if run_deep_verify {
                    indexing_service::scan_directory_deep_with_progress_cancel_deferred(
                        &conn,
                        &directory_for_scan,
                        allow_once,
                        stop_flag.as_ref(),
                        &enrichment_queue,
                        |processed, total| {
                            wait_for_user_idle(
                                &user_interaction_for_scan,
                                &app_for_scan,
                                &indexing_status_for_scan,
                            );
                            emitted_progress.store(true, Ordering::Relaxed);
                            emit_indexing_progress(
                                &app_for_scan,
                                &indexing_status_for_scan,
                                processed,
                                total,
                                progress_status(processed, total),
                            );
                        },
                    )
                } else {
                    indexing_service::scan_directory_pruned_with_progress_cancel_deferred(
                        &conn,
                        &directory_for_scan,
                        allow_once,
                        stop_flag.as_ref(),
                        &enrichment_queue,
                        |processed, total| {
                            wait_for_user_idle(
                                &user_interaction_for_scan,
                                &app_for_scan,
                                &indexing_status_for_scan,
                            );
                            emitted_progress.store(true, Ordering::Relaxed);
                            emit_indexing_progress(
                                &app_for_scan,
                                &indexing_status_for_scan,
                                processed,
                                total,
                                progress_status(processed, total),
                            );
                        },
                    )
                };

                if !cancelled {
                    if run_deep_verify {
                        if let Err(err) = mark_deep_verify_now(&conn) {
                            sentry::capture_message(
                                &format!("failed to persist deep verify timestamp: {err}"),
                                sentry::Level::Warning,
                            );
                        }
                    }

                    if let Err(err) = repository::cleanup_stale_embeddings(&conn) {
                        sentry::capture_message(
                            &format!("bootstrap stale embedding cleanup failed: {err}"),
                            sentry::Level::Warning,
                        );
                    }
                    if let Err(err) = repository::cleanup_stale_vec_by_age(
                        &conn,
                        indexing_service::VEC_RETENTION_DAYS,
                    ) {
                        sentry::capture_message(
                            &format!("bootstrap vec age cleanup failed: {err}"),
                            sentry::Level::Warning,
                        );
                    }
                }

                cancelled
            }
            Err(e) => {
                sentry::capture_message(
                    &format!("Indexing bootstrap DB open failed: {e}"),
                    sentry::Level::Warning,
                );
                emit_indexing_progress(&app_for_scan, &indexing_status_for_scan, 0, 0, "done");
                false
            }
        };

        if !emitted_progress.load(Ordering::Relaxed) {
            emit_indexing_progress(&app_for_scan, &indexing_status_for_scan, 0, 0, "done");
        }

        if cancelled {
            emit_indexing_progress(&app_for_scan, &indexing_status_for_scan, 0, 0, "done");
        }
    });
    watcher.attach_bootstrap_worker(bootstrap_worker);

    *handle_guard = Some(watcher);
    Ok(())
}

#[command]
pub fn get_indexing_status(state: State<'_, AppState>) -> Result<IndexingProgressState, AppError> {
    let guard = state
        .indexing_status
        .lock()
        .map_err(|e| AppError::General(e.to_string()))?;
    Ok(guard.clone())
}

#[command]
pub fn stop_indexing(app: AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    let mut handle_guard = state.watcher_handle.lock().unwrap();
    if let Some(handle) = handle_guard.take() {
        indexing_service::stop_watching(handle);
    }

    emit_indexing_progress(&app, &state.indexing_status, 0, 0, "done");
    Ok(())
}

#[command]
pub fn clear_indexed_data(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<repository::ClearIndexedDataReport, AppError> {
    {
        let mut handle_guard = state.watcher_handle.lock().unwrap();
        if let Some(handle) = handle_guard.take() {
            indexing_service::stop_watching(handle);
        }
    }

    let conn = state
        .db
        .lock()
        .map_err(|e| AppError::General(e.to_string()))?;
    let report = repository::clear_all_indexed_data(&conn)?;

    emit_indexing_progress(&app, &state.indexing_status, 0, 0, "done");
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::migrations;
    use crate::models::file_entry::FileEntry;

    #[test]
    fn progress_status_is_done_for_zero_total() {
        assert_eq!(progress_status(0, 0), "done");
    }

    #[test]
    fn progress_status_is_active_until_total_reached() {
        assert_eq!(progress_status(0, 10), "active");
        assert_eq!(progress_status(9, 10), "active");
        assert_eq!(progress_status(10, 10), "done");
    }

    fn test_conn() -> rusqlite::Connection {
        crate::data::register_sqlite_vec_extension();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        migrations::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn deep_verify_runs_when_setting_missing() {
        let conn = test_conn();
        assert!(should_run_deep_verify(&conn));
    }

    #[test]
    fn deep_verify_skips_when_recently_recorded() {
        let conn = test_conn();
        mark_deep_verify_now(&conn).unwrap();
        assert!(!should_run_deep_verify(&conn));
    }

    #[test]
    fn index_scope_maintenance_flags_round_trip() {
        let conn = test_conn();
        mark_index_scope_maintenance_started(&conn).unwrap();
        assert_eq!(
            repository::get_setting(&conn, INDEX_SCOPE_POLICY_MAINTENANCE_IN_PROGRESS_KEY)
                .unwrap()
                .as_deref(),
            Some("1")
        );

        clear_index_scope_maintenance_state(&conn);
        assert_eq!(
            repository::get_setting(&conn, INDEX_SCOPE_POLICY_MAINTENANCE_IN_PROGRESS_KEY)
                .unwrap()
                .as_deref(),
            Some("0")
        );
        assert_eq!(
            repository::get_setting(&conn, INDEX_SCOPE_POLICY_MAINTENANCE_STARTED_AT_KEY)
                .unwrap()
                .as_deref(),
            Some("0")
        );
    }

    #[test]
    fn index_scope_policy_prunes_out_of_scope_entries_once() {
        let conn = test_conn();
        let in_scope = FileEntry {
            path: "/tmp/scope-root/a.txt".to_string(),
            name: "a.txt".to_string(),
            extension: Some("txt".to_string()),
            mime_type: Some("text/plain".to_string()),
            size_bytes: Some(1),
            created_at: None,
            modified_at: Some("2025-01-01T00:00:00Z".to_string()),
            is_directory: false,
            parent_path: Some("/tmp/scope-root".to_string()),
        };
        let out_scope = FileEntry {
            path: "/tmp/other-root/b.txt".to_string(),
            name: "b.txt".to_string(),
            extension: Some("txt".to_string()),
            mime_type: Some("text/plain".to_string()),
            size_bytes: Some(1),
            created_at: None,
            modified_at: Some("2025-01-01T00:00:00Z".to_string()),
            is_directory: false,
            parent_path: Some("/tmp/other-root".to_string()),
        };
        repository::insert_file(&conn, &in_scope).unwrap();
        repository::insert_file(&conn, &out_scope).unwrap();

        let removed = maybe_apply_index_scope_policy(&conn, "/tmp/scope-root").unwrap();
        assert_eq!(removed, 1);
        assert!(repository::get_by_path(&conn, &in_scope.path)
            .unwrap()
            .is_some());
        assert!(repository::get_by_path(&conn, &out_scope.path)
            .unwrap()
            .is_none());

        let removed_again = maybe_apply_index_scope_policy(&conn, "/tmp/scope-root").unwrap();
        assert_eq!(removed_again, 0);
    }
}
