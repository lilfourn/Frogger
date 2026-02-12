use tauri::{command, State};

use crate::error::AppError;
use crate::models::search::SearchResult;
use crate::services::permission_service::{self, PermissionCapability};
use crate::services::search_service;
use crate::state::AppState;

#[command]
pub fn search(
    query: String,
    limit: Option<usize>,
    allow_once: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, AppError> {
    let allow_once = allow_once.unwrap_or(false);
    let conn = state.db.lock().unwrap();
    let policy = permission_service::load_policy_cache_entry(&conn, &state)?;
    let results = search_service::search(&conn, &query, limit.unwrap_or(20))?;
    Ok(results
        .into_iter()
        .filter(|r| {
            permission_service::enforce_with_cached_policy(
                &policy,
                &r.file_path,
                PermissionCapability::ContentScan,
                allow_once,
            )
            .is_ok()
        })
        .collect())
}
