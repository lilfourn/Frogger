use rusqlite::Connection;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::data::repository;
use crate::error::AppError;
use crate::shell::safety::is_protected_path;
use crate::state::{AppState, PermissionPolicyCacheEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    Deny,
    Ask,
    Allow,
}

impl PermissionMode {
    pub fn parse(value: &str) -> Result<Self, AppError> {
        match value {
            "deny" => Ok(Self::Deny),
            "ask" => Ok(Self::Ask),
            "allow" => Ok(Self::Allow),
            other => Err(AppError::General(format!(
                "invalid permission mode '{other}' (expected: deny|ask|allow)"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deny => "deny",
            Self::Ask => "ask",
            Self::Allow => "allow",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PermissionCapability {
    ContentScan,
    Modification,
    Ocr,
    Indexing,
}

impl PermissionCapability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ContentScan => "content_scan",
            Self::Modification => "modification",
            Self::Ocr => "ocr",
            Self::Indexing => "indexing",
        }
    }

    fn default_setting_key(self) -> &'static str {
        match self {
            Self::ContentScan => "permission_default_content_scan",
            Self::Modification => "permission_default_modification",
            Self::Ocr => "permission_default_ocr",
            Self::Indexing => "permission_default_indexing",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PermissionDefaults {
    pub content_scan_default: String,
    pub modification_default: String,
    pub ocr_default: String,
    pub indexing_default: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PermissionEvaluation {
    pub path: String,
    pub capability: String,
    pub mode: String,
    pub scope_path: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PermissionGrantTargetRequestItem {
    pub path: String,
    pub scope_path: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PermissionGrantTarget {
    pub path: String,
    pub scope_path: Option<String>,
    pub folder_target: String,
    pub exact_target: String,
    pub ambiguous: bool,
}

fn normalize_path(path: &str) -> String {
    let mut p = path.replace('\\', "/");
    while p.ends_with('/') && p.len() > 1 {
        p.pop();
    }
    p
}

fn parent_path(path: &str) -> Option<String> {
    if path == "/" {
        return None;
    }
    let index = path.rfind('/')?;
    if index == 0 {
        return Some("/".to_string());
    }
    Some(path[..index].to_string())
}

fn looks_like_file_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.starts_with('.')
        || name.ends_with('.')
    {
        return false;
    }
    let Some((_, ext)) = name.rsplit_once('.') else {
        return false;
    };
    !ext.is_empty() && ext.len() <= 12 && ext.chars().all(|ch| ch.is_ascii_alphanumeric())
}

fn infer_folder_target(path: &str) -> String {
    let normalized = normalize_path(path);
    if normalized.is_empty() {
        return normalized;
    }

    if let Ok(metadata) = std::fs::metadata(Path::new(path)) {
        if metadata.is_dir() {
            return normalized;
        }
        if metadata.is_file() {
            return parent_path(&normalized).unwrap_or(normalized);
        }
    }

    if looks_like_file_path(&normalized) {
        return parent_path(&normalized).unwrap_or(normalized);
    }

    normalized
}

pub fn resolve_permission_grant_targets(
    items: &[PermissionGrantTargetRequestItem],
) -> Vec<PermissionGrantTarget> {
    items
        .iter()
        .map(|item| {
            let exact_target = normalize_path(&item.path);
            let folder_target = match item.scope_path.as_deref() {
                Some(scope_path) => normalize_path(scope_path),
                None => infer_folder_target(&item.path),
            };
            let ambiguous = item.scope_path.is_none()
                && !folder_target.is_empty()
                && !exact_target.is_empty()
                && folder_target != exact_target;

            PermissionGrantTarget {
                path: item.path.clone(),
                scope_path: item.scope_path.clone(),
                folder_target,
                exact_target,
                ambiguous,
            }
        })
        .collect()
}

fn scope_matches_path(path: &str, scope: &str) -> bool {
    let path = normalize_path(path);
    let scope = normalize_path(scope);
    if scope == "/" {
        return path.starts_with('/');
    }
    path == scope || path.starts_with(&(scope + "/"))
}

pub fn resolve_default_mode(
    conn: &Connection,
    capability: PermissionCapability,
) -> Result<PermissionMode, AppError> {
    if matches!(capability, PermissionCapability::Indexing) {
        return Ok(PermissionMode::Allow);
    }

    let key = capability.default_setting_key();
    match repository::get_setting(conn, key)? {
        Some(value) => PermissionMode::parse(&value),
        None => Ok(PermissionMode::Ask),
    }
}

pub fn get_defaults(conn: &Connection) -> Result<PermissionDefaults, AppError> {
    Ok(PermissionDefaults {
        content_scan_default: resolve_default_mode(conn, PermissionCapability::ContentScan)?
            .as_str()
            .to_string(),
        modification_default: resolve_default_mode(conn, PermissionCapability::Modification)?
            .as_str()
            .to_string(),
        ocr_default: resolve_default_mode(conn, PermissionCapability::Ocr)?
            .as_str()
            .to_string(),
        indexing_default: resolve_default_mode(conn, PermissionCapability::Indexing)?
            .as_str()
            .to_string(),
    })
}

pub fn set_defaults(conn: &Connection, defaults: &PermissionDefaults) -> Result<(), AppError> {
    PermissionMode::parse(&defaults.content_scan_default)?;
    PermissionMode::parse(&defaults.modification_default)?;
    PermissionMode::parse(&defaults.ocr_default)?;
    PermissionMode::parse(&defaults.indexing_default)?;

    repository::set_setting(
        conn,
        PermissionCapability::ContentScan.default_setting_key(),
        &defaults.content_scan_default,
    )?;
    repository::set_setting(
        conn,
        PermissionCapability::Modification.default_setting_key(),
        &defaults.modification_default,
    )?;
    repository::set_setting(
        conn,
        PermissionCapability::Ocr.default_setting_key(),
        &defaults.ocr_default,
    )?;
    repository::set_setting(
        conn,
        PermissionCapability::Indexing.default_setting_key(),
        &defaults.indexing_default,
    )?;

    Ok(())
}

fn default_mode_from_cache(
    cache: &PermissionPolicyCacheEntry,
    capability: PermissionCapability,
) -> Result<PermissionMode, AppError> {
    if matches!(capability, PermissionCapability::Indexing) {
        return Ok(PermissionMode::Allow);
    }

    let value = match capability {
        PermissionCapability::ContentScan => &cache.content_scan_default,
        PermissionCapability::Modification => &cache.modification_default,
        PermissionCapability::Ocr => &cache.ocr_default,
        PermissionCapability::Indexing => &cache.indexing_default,
    };
    PermissionMode::parse(value)
}

fn build_policy_cache_entry(
    conn: &Connection,
    version: u64,
) -> Result<PermissionPolicyCacheEntry, AppError> {
    let defaults = get_defaults(conn)?;
    let scopes = repository::get_permission_scopes(conn)?;
    Ok(PermissionPolicyCacheEntry {
        version,
        scopes,
        content_scan_default: defaults.content_scan_default,
        modification_default: defaults.modification_default,
        ocr_default: defaults.ocr_default,
        indexing_default: defaults.indexing_default,
    })
}

pub fn load_policy_cache_entry(
    conn: &Connection,
    state: &AppState,
) -> Result<Arc<PermissionPolicyCacheEntry>, AppError> {
    let expected_version = state.permission_policy_version.load(Ordering::Acquire);

    if let Ok(cache_guard) = state.permission_policy_cache.read() {
        if let Some(cache) = cache_guard.as_ref() {
            if cache.version == expected_version {
                return Ok(cache.clone());
            }
        }
    }

    let fresh = Arc::new(build_policy_cache_entry(conn, expected_version)?);

    if let Ok(mut cache_guard) = state.permission_policy_cache.write() {
        let latest_version = state.permission_policy_version.load(Ordering::Acquire);
        if latest_version == expected_version {
            *cache_guard = Some(fresh.clone());
        }
    }

    Ok(fresh)
}

pub fn invalidate_policy_cache(state: &AppState) {
    state
        .permission_policy_version
        .fetch_add(1, Ordering::AcqRel);
    if let Ok(mut cache_guard) = state.permission_policy_cache.write() {
        *cache_guard = None;
    }
}

pub fn evaluate_with_cached_policy(
    cache: &PermissionPolicyCacheEntry,
    path: &str,
    capability: PermissionCapability,
) -> Result<PermissionEvaluation, AppError> {
    let default_mode = default_mode_from_cache(cache, capability)?;
    evaluate_with_scopes(&cache.scopes, path, capability, default_mode)
}

pub fn enforce_with_cached_policy(
    cache: &PermissionPolicyCacheEntry,
    path: &str,
    capability: PermissionCapability,
    allow_once: bool,
) -> Result<(), AppError> {
    let evaluation = evaluate_with_cached_policy(cache, path, capability)?;
    enforce_evaluation(&evaluation, allow_once)
}

pub fn enforce_cached(
    conn: &Connection,
    state: &AppState,
    path: &str,
    capability: PermissionCapability,
    allow_once: bool,
) -> Result<(), AppError> {
    let cache = load_policy_cache_entry(conn, state)?;
    enforce_with_cached_policy(&cache, path, capability, allow_once)
}

pub fn evaluate(
    conn: &Connection,
    path: &str,
    capability: PermissionCapability,
) -> Result<PermissionEvaluation, AppError> {
    if is_protected_path(path) {
        return Ok(PermissionEvaluation {
            path: path.to_string(),
            capability: capability.as_str().to_string(),
            mode: PermissionMode::Deny.as_str().to_string(),
            scope_path: None,
        });
    }

    let scopes = repository::get_permission_scopes(conn)?;
    let default_mode = resolve_default_mode(conn, capability)?;
    evaluate_with_scopes(&scopes, path, capability, default_mode)
}

pub fn evaluate_with_scopes(
    scopes: &[repository::PermissionScope],
    path: &str,
    capability: PermissionCapability,
    default_mode: PermissionMode,
) -> Result<PermissionEvaluation, AppError> {
    if is_protected_path(path) {
        return Ok(PermissionEvaluation {
            path: path.to_string(),
            capability: capability.as_str().to_string(),
            mode: PermissionMode::Deny.as_str().to_string(),
            scope_path: None,
        });
    }

    if matches!(capability, PermissionCapability::Indexing) {
        return Ok(PermissionEvaluation {
            path: path.to_string(),
            capability: capability.as_str().to_string(),
            mode: PermissionMode::Allow.as_str().to_string(),
            scope_path: None,
        });
    }

    for scope in scopes {
        if scope_matches_path(path, &scope.directory_path) {
            let mode = match capability {
                PermissionCapability::ContentScan => scope.content_scan_mode.as_str(),
                PermissionCapability::Modification => scope.modification_mode.as_str(),
                PermissionCapability::Ocr => scope.ocr_mode.as_str(),
                PermissionCapability::Indexing => scope.indexing_mode.as_str(),
            };
            let parsed = PermissionMode::parse(mode)?;
            return Ok(PermissionEvaluation {
                path: path.to_string(),
                capability: capability.as_str().to_string(),
                mode: parsed.as_str().to_string(),
                scope_path: Some(scope.directory_path.clone()),
            });
        }
    }

    Ok(PermissionEvaluation {
        path: path.to_string(),
        capability: capability.as_str().to_string(),
        mode: default_mode.as_str().to_string(),
        scope_path: None,
    })
}

pub fn enforce(
    conn: &Connection,
    path: &str,
    capability: PermissionCapability,
    allow_once: bool,
) -> Result<(), AppError> {
    let evaluation = evaluate(conn, path, capability)?;
    enforce_evaluation(&evaluation, allow_once)
}

pub fn enforce_with_scopes(
    scopes: &[repository::PermissionScope],
    path: &str,
    capability: PermissionCapability,
    allow_once: bool,
    default_mode: PermissionMode,
) -> Result<(), AppError> {
    let evaluation = evaluate_with_scopes(scopes, path, capability, default_mode)?;
    enforce_evaluation(&evaluation, allow_once)
}

fn enforce_evaluation(evaluation: &PermissionEvaluation, allow_once: bool) -> Result<(), AppError> {
    match PermissionMode::parse(&evaluation.mode)? {
        PermissionMode::Allow => Ok(()),
        PermissionMode::Deny => Err(AppError::General(format!(
            "permission denied for {} on {}",
            evaluation.capability, evaluation.path
        ))),
        PermissionMode::Ask => {
            if allow_once {
                Ok(())
            } else {
                Err(AppError::General(format!(
                    "permission confirmation required for {} on {}",
                    evaluation.capability, evaluation.path
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::migrations;
    use crate::state::IndexingProgressState;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicU64};
    use std::sync::{Arc, Mutex, RwLock};

    fn test_conn(default_mode: &str) -> Connection {
        crate::data::register_sqlite_vec_extension();
        let conn = Connection::open_in_memory().unwrap();
        migrations::run_migrations(&conn).unwrap();
        repository::set_setting(&conn, "permission_default_content_scan", default_mode).unwrap();
        repository::set_setting(&conn, "permission_default_modification", default_mode).unwrap();
        repository::set_setting(&conn, "permission_default_ocr", default_mode).unwrap();
        repository::set_setting(&conn, "permission_default_indexing", default_mode).unwrap();
        conn
    }

    fn test_state() -> AppState {
        AppState {
            db: Mutex::new(Connection::open_in_memory().unwrap()),
            db_path: PathBuf::new(),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            organize_cancel_flags: Mutex::new(HashMap::new()),
            watcher_handle: Mutex::new(None),
            indexing_status: Arc::new(Mutex::new(IndexingProgressState {
                processed: 0,
                total: 0,
                status: "done".to_string(),
            })),
            permission_policy_cache: RwLock::new(None),
            permission_policy_version: AtomicU64::new(1),
        }
    }

    #[test]
    fn test_protected_paths_are_denied_even_when_defaults_allow() {
        let conn = test_conn("allow");

        let evaluation =
            evaluate(&conn, "/System/Library", PermissionCapability::ContentScan).unwrap();

        assert_eq!(evaluation.mode, "deny");
        assert!(
            enforce(
                &conn,
                "/System/Library",
                PermissionCapability::ContentScan,
                true
            )
            .is_err(),
            "protected path should stay denied even with allow_once"
        );
    }

    #[test]
    fn test_protected_paths_override_allow_scopes() {
        let conn = test_conn("allow");
        repository::upsert_permission_scope(
            &conn,
            "/Applications",
            "allow",
            "allow",
            "allow",
            "allow",
        )
        .unwrap();

        let evaluation = evaluate(
            &conn,
            "/Applications/MyApp.app",
            PermissionCapability::Modification,
        )
        .unwrap();

        assert_eq!(evaluation.mode, "deny");
    }

    #[test]
    fn test_non_protected_paths_respect_defaults() {
        let conn = test_conn("allow");
        assert!(enforce(
            &conn,
            "/Users/test/Documents",
            PermissionCapability::ContentScan,
            false
        )
        .is_ok());
    }

    #[test]
    fn test_indexing_is_always_allowed_for_non_protected_paths() {
        let conn = test_conn("deny");
        repository::upsert_permission_scope(&conn, "/Users/test", "deny", "deny", "deny", "deny")
            .unwrap();

        let evaluation = evaluate(
            &conn,
            "/Users/test/Documents/file.txt",
            PermissionCapability::Indexing,
        )
        .unwrap();

        assert_eq!(evaluation.mode, "allow");
        assert!(enforce(
            &conn,
            "/Users/test/Documents/file.txt",
            PermissionCapability::Indexing,
            false,
        )
        .is_ok());
    }

    #[test]
    fn test_indexing_remains_denied_for_protected_paths() {
        let conn = test_conn("allow");

        let evaluation =
            evaluate(&conn, "/System/Library", PermissionCapability::Indexing).unwrap();

        assert_eq!(evaluation.mode, "deny");
        assert!(enforce(
            &conn,
            "/System/Library",
            PermissionCapability::Indexing,
            true,
        )
        .is_err());
    }

    #[test]
    fn test_resolve_permission_grant_targets_marks_file_paths_ambiguous() {
        let targets = resolve_permission_grant_targets(&[PermissionGrantTargetRequestItem {
            path: "/Users/test/Documents/report.md".to_string(),
            scope_path: None,
        }]);

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].folder_target, "/Users/test/Documents");
        assert_eq!(targets[0].exact_target, "/Users/test/Documents/report.md");
        assert!(targets[0].ambiguous);
    }

    #[test]
    fn test_resolve_permission_grant_targets_prefers_scope_path_when_present() {
        let targets = resolve_permission_grant_targets(&[PermissionGrantTargetRequestItem {
            path: "/Users/test/Documents/report.md".to_string(),
            scope_path: Some("/Users/test/Documents".to_string()),
        }]);

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].folder_target, "/Users/test/Documents");
        assert!(!targets[0].ambiguous);
    }

    #[test]
    fn test_load_policy_cache_entry_reuses_cached_arc() {
        let conn = test_conn("ask");
        let state = test_state();

        let first = load_policy_cache_entry(&conn, &state).unwrap();
        let second = load_policy_cache_entry(&conn, &state).unwrap();

        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn test_invalidate_policy_cache_refreshes_defaults() {
        let conn = test_conn("ask");
        let state = test_state();

        let before = load_policy_cache_entry(&conn, &state).unwrap();
        assert_eq!(before.content_scan_default, "ask");

        repository::set_setting(&conn, "permission_default_content_scan", "allow").unwrap();

        let stale = load_policy_cache_entry(&conn, &state).unwrap();
        assert_eq!(stale.content_scan_default, "ask");

        invalidate_policy_cache(&state);

        let refreshed = load_policy_cache_entry(&conn, &state).unwrap();
        assert_eq!(refreshed.content_scan_default, "allow");
    }
}
