use tauri::{command, State};

use crate::data::repository::{
    self, AuditLogEntry, PermissionScope, PermissionScopeNormalizationReport,
};
use crate::error::AppError;
use crate::services::permission_service::{
    self, PermissionCapability, PermissionDefaults, PermissionEvaluation,
    PermissionGrantTargetRequestItem, PermissionMode,
};
use crate::state::AppState;

const KEYRING_SERVICE: &str = "frogger";
const KEYRING_USER: &str = "api_key";

const BLOCKED_SETTINGS_KEYS: &[&str] = &["api_key"];

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionCheckRequest {
    pub action: String,
    pub paths: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct PermissionCheckResponse {
    pub decision: String,
    pub blocked: Vec<PermissionEvaluation>,
}

fn action_capability(action: &str) -> Result<PermissionCapability, AppError> {
    match action {
        "list_directory"
        | "read_file_text"
        | "open_file"
        | "find_large_files"
        | "find_old_files"
        | "find_duplicates"
        | "detect_project_type"
        | "search"
        | "send_chat"
        | "send_organize_plan"
        | "send_organize_execute" => Ok(PermissionCapability::ContentScan),
        "create_directory"
        | "rename_file"
        | "move_files"
        | "copy_files"
        | "delete_files"
        | "send_organize_apply"
        | "copy_files_with_progress"
        | "undo_operation"
        | "redo_operation" => Ok(PermissionCapability::Modification),
        "start_indexing" => Ok(PermissionCapability::Indexing),
        "ocr_process" => Ok(PermissionCapability::Ocr),
        _ => Err(AppError::General(format!(
            "unknown action for permission preflight: {action}"
        ))),
    }
}

fn keyring_entry() -> Result<keyring::Entry, AppError> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| AppError::General(format!("Keychain error: {e}")))
}

#[command]
pub fn save_api_key(key: String) -> Result<(), AppError> {
    keyring_entry()?
        .set_password(&key)
        .map_err(|e| AppError::General(format!("Failed to save API key: {e}")))
}

#[command]
pub fn has_api_key() -> Result<bool, AppError> {
    match keyring_entry()?.get_password() {
        Ok(_) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(e) => Err(AppError::General(format!("Keychain error: {e}"))),
    }
}

#[command]
pub fn delete_api_key() -> Result<(), AppError> {
    match keyring_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::General(format!("Failed to delete API key: {e}"))),
    }
}

#[command]
pub fn get_setting(key: String, state: State<'_, AppState>) -> Result<Option<String>, AppError> {
    if BLOCKED_SETTINGS_KEYS.contains(&key.as_str()) {
        return Err(AppError::General(format!(
            "Setting '{key}' cannot be accessed via generic endpoint"
        )));
    }
    let conn = state
        .db
        .lock()
        .map_err(|e| AppError::General(e.to_string()))?;
    repository::get_setting(&conn, &key)
}

#[command]
pub fn set_setting(key: String, value: String, state: State<'_, AppState>) -> Result<(), AppError> {
    if BLOCKED_SETTINGS_KEYS.contains(&key.as_str()) {
        return Err(AppError::General(format!(
            "Setting '{key}' cannot be modified via generic endpoint"
        )));
    }
    let conn = state
        .db
        .lock()
        .map_err(|e| AppError::General(e.to_string()))?;
    repository::set_setting(&conn, &key, &value)?;
    if key.starts_with("permission_default_") {
        permission_service::invalidate_policy_cache(&state);
    }
    Ok(())
}

// --- Permission scopes ---

#[command]
pub fn get_permission_scopes(state: State<'_, AppState>) -> Result<Vec<PermissionScope>, AppError> {
    let conn = state
        .db
        .lock()
        .map_err(|e| AppError::General(e.to_string()))?;
    repository::get_permission_scopes(&conn)
}

#[command]
pub fn check_permission_request(
    request: PermissionCheckRequest,
    state: State<'_, AppState>,
) -> Result<PermissionCheckResponse, AppError> {
    let conn = state
        .db
        .lock()
        .map_err(|e| AppError::General(e.to_string()))?;
    let capability = action_capability(&request.action)?;
    let cache = permission_service::load_policy_cache_entry(&conn, &state)?;

    let mut blocked = Vec::new();
    let mut has_deny = false;
    let mut has_ask = false;

    for path in request.paths {
        let eval = permission_service::evaluate_with_cached_policy(&cache, &path, capability)?;
        match eval.mode.as_str() {
            "deny" => {
                has_deny = true;
                blocked.push(eval);
            }
            "ask" => {
                has_ask = true;
                blocked.push(eval);
            }
            _ => {}
        }
    }

    let decision = if has_deny {
        "deny"
    } else if has_ask {
        "ask"
    } else {
        "allow"
    };

    Ok(PermissionCheckResponse {
        decision: decision.to_string(),
        blocked,
    })
}

#[command]
pub fn upsert_permission_scope(
    directory_path: String,
    content_scan_mode: String,
    modification_mode: String,
    ocr_mode: String,
    indexing_mode: String,
    state: State<'_, AppState>,
) -> Result<i64, AppError> {
    let content_scan_mode = PermissionMode::parse(&content_scan_mode)?.as_str();
    let modification_mode = PermissionMode::parse(&modification_mode)?.as_str();
    let ocr_mode = PermissionMode::parse(&ocr_mode)?.as_str();
    let indexing_mode = PermissionMode::parse(&indexing_mode)?.as_str();

    let conn = state
        .db
        .lock()
        .map_err(|e| AppError::General(e.to_string()))?;
    let result = repository::upsert_permission_scope(
        &conn,
        &directory_path,
        content_scan_mode,
        modification_mode,
        ocr_mode,
        indexing_mode,
    )?;
    permission_service::invalidate_policy_cache(&state);
    Ok(result)
}

#[command]
pub fn get_permission_defaults(state: State<'_, AppState>) -> Result<PermissionDefaults, AppError> {
    let conn = state
        .db
        .lock()
        .map_err(|e| AppError::General(e.to_string()))?;
    permission_service::get_defaults(&conn)
}

#[command]
pub fn set_permission_defaults(
    content_scan_default: String,
    modification_default: String,
    ocr_default: String,
    indexing_default: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let conn = state
        .db
        .lock()
        .map_err(|e| AppError::General(e.to_string()))?;
    permission_service::set_defaults(
        &conn,
        &PermissionDefaults {
            content_scan_default,
            modification_default,
            ocr_default,
            indexing_default,
        },
    )?;
    permission_service::invalidate_policy_cache(&state);
    Ok(())
}

#[command]
pub fn delete_permission_scope(id: i64, state: State<'_, AppState>) -> Result<usize, AppError> {
    let conn = state
        .db
        .lock()
        .map_err(|e| AppError::General(e.to_string()))?;
    let count = repository::delete_permission_scope(&conn, id)?;
    permission_service::invalidate_policy_cache(&state);
    Ok(count)
}

#[command]
pub fn normalize_permission_scopes(
    state: State<'_, AppState>,
) -> Result<PermissionScopeNormalizationReport, AppError> {
    let conn = state
        .db
        .lock()
        .map_err(|e| AppError::General(e.to_string()))?;
    let report = repository::normalize_permission_scopes(&conn)?;
    permission_service::invalidate_policy_cache(&state);
    Ok(report)
}

#[command]
pub fn resolve_permission_grant_targets(
    items: Vec<PermissionGrantTargetRequestItem>,
) -> Vec<permission_service::PermissionGrantTarget> {
    permission_service::resolve_permission_grant_targets(&items)
}

// --- Audit log ---

#[command]
pub fn get_audit_log(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<AuditLogEntry>, AppError> {
    let conn = state
        .db
        .lock()
        .map_err(|e| AppError::General(e.to_string()))?;
    repository::get_audit_log(&conn, limit.unwrap_or(100))
}
