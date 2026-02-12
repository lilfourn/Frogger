use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use crate::data::repository::PermissionScope;
use crate::services::indexing_service::IndexingHandle;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IndexingProgressState {
    pub processed: usize,
    pub total: usize,
    pub status: String,
}

#[derive(Debug)]
pub struct PermissionPolicyCacheEntry {
    pub version: u64,
    pub scopes: Vec<PermissionScope>,
    pub content_scan_default: String,
    pub modification_default: String,
    pub ocr_default: String,
    pub indexing_default: String,
}

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub db_path: PathBuf,
    pub cancel_flag: Arc<AtomicBool>,
    pub organize_cancel_flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
    pub watcher_handle: Mutex<Option<IndexingHandle>>,
    pub indexing_status: Arc<Mutex<IndexingProgressState>>,
    pub permission_policy_cache: RwLock<Option<Arc<PermissionPolicyCacheEntry>>>,
    pub permission_policy_version: AtomicU64,
}

impl AppState {
    pub fn reset_organize_cancel_flag(&self, session_id: &str) -> Arc<AtomicBool> {
        let mut flags = self
            .organize_cancel_flags
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let flag = flags
            .entry(session_id.to_string())
            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .clone();
        flag.store(false, Ordering::Relaxed);
        flag
    }

    pub fn mark_organize_cancelled(&self, session_id: Option<&str>) {
        let flags = self
            .organize_cancel_flags
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(id) = session_id {
            if let Some(flag) = flags.get(id) {
                flag.store(true, Ordering::Relaxed);
            }
            return;
        }

        for flag in flags.values() {
            flag.store(true, Ordering::Relaxed);
        }
    }

    pub fn clear_organize_cancel_flag(&self, session_id: &str) {
        let mut flags = self
            .organize_cancel_flags
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        flags.remove(session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> AppState {
        AppState {
            db: Mutex::new(rusqlite::Connection::open_in_memory().unwrap()),
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
    fn cancel_scopes_are_session_specific() {
        let state = test_state();
        let flag_a = state.reset_organize_cancel_flag("a");
        let flag_b = state.reset_organize_cancel_flag("b");

        state.mark_organize_cancelled(Some("a"));

        assert!(flag_a.load(Ordering::Relaxed));
        assert!(!flag_b.load(Ordering::Relaxed));
    }

    #[test]
    fn cancel_without_session_marks_all_active_sessions() {
        let state = test_state();
        let flag_a = state.reset_organize_cancel_flag("a");
        let flag_b = state.reset_organize_cancel_flag("b");

        state.mark_organize_cancelled(None);

        assert!(flag_a.load(Ordering::Relaxed));
        assert!(flag_b.load(Ordering::Relaxed));
    }
}
