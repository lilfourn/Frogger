use std::sync::{Arc, Mutex};
use tauri::{command, State};

use crate::data::migrations;
use crate::error::AppError;
use crate::services::indexing_service;
use crate::state::AppState;

#[command]
pub fn start_indexing(directory: String, state: State<'_, AppState>) -> Result<(), AppError> {
    let mut handle_guard = state.watcher_handle.lock().unwrap();
    if handle_guard.is_some() {
        return Err(AppError::Watcher("indexing already running".to_string()));
    }

    let conn =
        rusqlite::Connection::open(&state.db_path).map_err(|e| AppError::Watcher(e.to_string()))?;
    migrations::run_migrations(&conn)?;
    let db = Arc::new(Mutex::new(conn));

    let watcher = indexing_service::start_watching(db, &directory)?;
    *handle_guard = Some(watcher);
    Ok(())
}

#[command]
pub fn stop_indexing(state: State<'_, AppState>) -> Result<(), AppError> {
    let mut handle_guard = state.watcher_handle.lock().unwrap();
    match handle_guard.take() {
        Some(handle) => {
            indexing_service::stop_watching(handle);
            Ok(())
        }
        None => Err(AppError::Watcher("no indexing running".to_string())),
    }
}
