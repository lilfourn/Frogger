use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{command, AppHandle, Emitter, State};

use crate::data::migrations;
use crate::error::AppError;
use crate::services::indexing_service;
use crate::services::permission_service::{self, PermissionCapability};
use crate::state::{AppState, IndexingProgressState};

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

#[command]
pub fn start_indexing(
    directory: String,
    allow_once: Option<bool>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let allow_once = allow_once.unwrap_or(false);
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

    let conn =
        rusqlite::Connection::open(&state.db_path).map_err(|e| AppError::Watcher(e.to_string()))?;
    migrations::run_migrations(&conn)?;
    let db = Arc::new(Mutex::new(conn));
    let indexing_status = state.indexing_status.clone();
    let mut watcher = indexing_service::start_watching(
        db,
        state.db_path.to_string_lossy().to_string(),
        &directory,
        allow_once,
    )?;
    emit_indexing_progress(&app, &indexing_status, 0, 0, "starting");

    let stop_flag = watcher.stop_flag();
    let enrichment_queue = watcher.enrichment_queue();
    let db_path_for_scan = state.db_path.clone();
    let directory_for_scan = directory.clone();
    let app_for_scan = app.clone();
    let indexing_status_for_scan = indexing_status.clone();
    let bootstrap_worker = std::thread::spawn(move || {
        let emitted_progress = AtomicBool::new(false);
        let cancelled = match rusqlite::Connection::open(&db_path_for_scan) {
            Ok(conn) => {
                let _ = conn.busy_timeout(std::time::Duration::from_secs(5));
                indexing_service::scan_directory_deep_with_progress_cancel_deferred(
                    &conn,
                    &directory_for_scan,
                    allow_once,
                    stop_flag.as_ref(),
                    &enrichment_queue,
                    |processed, total| {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
