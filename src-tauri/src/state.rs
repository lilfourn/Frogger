use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use crate::data::repository::PermissionScope;
use crate::services::indexing_service::IndexingHandle;

const MAX_ORGANIZE_STATUS_ENTRIES: usize = 256;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IndexingProgressState {
    pub processed: usize,
    pub total: usize,
    pub status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OrganizeProgressPhase {
    Indexing,
    Planning,
    Applying,
    Done,
    Cancelled,
    Error,
}

impl OrganizeProgressPhase {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Done | Self::Cancelled | Self::Error)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizeProgressState {
    pub session_id: String,
    pub root_path: String,
    pub phase: OrganizeProgressPhase,
    pub processed: usize,
    pub total: usize,
    pub percent: usize,
    pub combined_percent: usize,
    pub message: String,
    pub sequence: u64,
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
    pub file_operation_cancel_flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
    pub organize_cancel_flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
    pub organize_status: Mutex<HashMap<String, OrganizeProgressState>>,
    pub organize_progress_sequence: AtomicU64,
    pub watcher_handle: Mutex<Option<IndexingHandle>>,
    pub indexing_status: Arc<Mutex<IndexingProgressState>>,
    pub last_user_interaction_at: Arc<AtomicI64>,
    pub permission_policy_cache: RwLock<Option<Arc<PermissionPolicyCacheEntry>>>,
    pub permission_policy_version: AtomicU64,
}

impl AppState {
    pub fn reset_file_operation_cancel_flag(&self, operation_id: &str) -> Arc<AtomicBool> {
        let mut flags = self
            .file_operation_cancel_flags
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let flag = flags
            .entry(operation_id.to_string())
            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .clone();
        flag.store(false, Ordering::Relaxed);
        flag
    }

    pub fn mark_file_operation_cancelled(&self, operation_id: Option<&str>) {
        let flags = self
            .file_operation_cancel_flags
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(id) = operation_id {
            if let Some(flag) = flags.get(id) {
                flag.store(true, Ordering::Relaxed);
            }
            return;
        }

        for flag in flags.values() {
            flag.store(true, Ordering::Relaxed);
        }
    }

    pub fn clear_file_operation_cancel_flag(&self, operation_id: &str) {
        let mut flags = self
            .file_operation_cancel_flags
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        flags.remove(operation_id);
    }

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

    pub fn next_organize_progress_sequence(&self) -> u64 {
        self.organize_progress_sequence
            .fetch_add(1, Ordering::AcqRel)
            + 1
    }

    pub fn set_organize_status(&self, status: OrganizeProgressState) {
        let mut statuses = self
            .organize_status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        statuses.insert(status.session_id.clone(), status);
        Self::prune_organize_statuses(&mut statuses);
    }

    pub fn get_organize_status(&self, session_id: &str) -> Option<OrganizeProgressState> {
        let statuses = self
            .organize_status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        statuses.get(session_id).cloned()
    }

    pub fn clear_organize_status(&self, session_id: &str) {
        let mut statuses = self
            .organize_status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        statuses.remove(session_id);
    }

    fn prune_organize_statuses(statuses: &mut HashMap<String, OrganizeProgressState>) {
        if statuses.len() <= MAX_ORGANIZE_STATUS_ENTRIES {
            return;
        }

        let mut terminal_keys = statuses
            .iter()
            .filter_map(|(key, progress)| {
                if progress.phase.is_terminal() {
                    Some((key.clone(), progress.sequence))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        terminal_keys.sort_by_key(|(_, sequence)| *sequence);

        for (key, _) in terminal_keys {
            if statuses.len() <= MAX_ORGANIZE_STATUS_ENTRIES {
                break;
            }
            statuses.remove(&key);
        }

        if statuses.len() <= MAX_ORGANIZE_STATUS_ENTRIES {
            return;
        }

        let mut all_keys = statuses
            .iter()
            .map(|(key, progress)| (key.clone(), progress.sequence))
            .collect::<Vec<_>>();
        all_keys.sort_by_key(|(_, sequence)| *sequence);

        for (key, _) in all_keys {
            if statuses.len() <= MAX_ORGANIZE_STATUS_ENTRIES {
                break;
            }
            statuses.remove(&key);
        }
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
            file_operation_cancel_flags: Mutex::new(HashMap::new()),
            organize_cancel_flags: Mutex::new(HashMap::new()),
            organize_status: Mutex::new(HashMap::new()),
            organize_progress_sequence: AtomicU64::new(0),
            watcher_handle: Mutex::new(None),
            indexing_status: Arc::new(Mutex::new(IndexingProgressState {
                processed: 0,
                total: 0,
                status: "done".to_string(),
            })),
            last_user_interaction_at: Arc::new(AtomicI64::new(0)),
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

    #[test]
    fn file_operation_cancel_scopes_are_operation_specific() {
        let state = test_state();
        let flag_a = state.reset_file_operation_cancel_flag("copy-a");
        let flag_b = state.reset_file_operation_cancel_flag("copy-b");

        state.mark_file_operation_cancelled(Some("copy-a"));

        assert!(flag_a.load(Ordering::Relaxed));
        assert!(!flag_b.load(Ordering::Relaxed));
    }

    #[test]
    fn file_operation_cancel_without_id_marks_all_active_operations() {
        let state = test_state();
        let flag_a = state.reset_file_operation_cancel_flag("copy-a");
        let flag_b = state.reset_file_operation_cancel_flag("copy-b");

        state.mark_file_operation_cancelled(None);

        assert!(flag_a.load(Ordering::Relaxed));
        assert!(flag_b.load(Ordering::Relaxed));
    }

    #[test]
    fn organize_status_round_trips_by_session() {
        let state = test_state();
        let status = OrganizeProgressState {
            session_id: "session-1".to_string(),
            root_path: "/tmp".to_string(),
            phase: OrganizeProgressPhase::Indexing,
            processed: 12,
            total: 100,
            percent: 12,
            combined_percent: 1,
            message: "Indexing files 12/100".to_string(),
            sequence: 4,
        };

        state.set_organize_status(status.clone());
        let fetched = state.get_organize_status("session-1");
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().message, status.message);

        state.clear_organize_status("session-1");
        assert!(state.get_organize_status("session-1").is_none());
    }

    #[test]
    fn organize_status_prunes_old_terminal_entries() {
        let state = test_state();

        for idx in 0..(MAX_ORGANIZE_STATUS_ENTRIES + 5) {
            state.set_organize_status(OrganizeProgressState {
                session_id: format!("session-{idx}"),
                root_path: "/tmp".to_string(),
                phase: OrganizeProgressPhase::Done,
                processed: 100,
                total: 100,
                percent: 100,
                combined_percent: 100,
                message: "Done".to_string(),
                sequence: (idx + 1) as u64,
            });
        }

        let statuses = state
            .organize_status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert_eq!(statuses.len(), MAX_ORGANIZE_STATUS_ENTRIES);
        assert!(!statuses.contains_key("session-0"));
        assert!(statuses.contains_key(&format!("session-{}", MAX_ORGANIZE_STATUS_ENTRIES + 4)));
    }

    #[test]
    fn organize_status_prefers_removing_terminal_before_active() {
        let state = test_state();

        for idx in 0..MAX_ORGANIZE_STATUS_ENTRIES {
            state.set_organize_status(OrganizeProgressState {
                session_id: format!("terminal-{idx}"),
                root_path: "/tmp".to_string(),
                phase: OrganizeProgressPhase::Done,
                processed: 100,
                total: 100,
                percent: 100,
                combined_percent: 100,
                message: "Done".to_string(),
                sequence: (idx + 1) as u64,
            });
        }

        state.set_organize_status(OrganizeProgressState {
            session_id: "active-session".to_string(),
            root_path: "/tmp".to_string(),
            phase: OrganizeProgressPhase::Planning,
            processed: 20,
            total: 100,
            percent: 20,
            combined_percent: 40,
            message: "Planning".to_string(),
            sequence: 1,
        });

        let statuses = state
            .organize_status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert_eq!(statuses.len(), MAX_ORGANIZE_STATUS_ENTRIES);
        assert!(statuses.contains_key("active-session"));
    }
}
