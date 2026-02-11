use tauri::{command, State};

use crate::error::AppError;
use crate::models::search::SearchResult;
use crate::services::search_service;
use crate::state::AppState;

#[command]
pub fn search(
    query: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, AppError> {
    let conn = state.db.lock().unwrap();
    search_service::hybrid_search(&conn, &query, limit.unwrap_or(20))
}
