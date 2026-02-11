use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use crate::services::indexing_service::IndexingHandle;

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub db_path: PathBuf,
    pub cancel_flag: Arc<AtomicBool>,
    pub watcher_handle: Mutex<Option<IndexingHandle>>,
}
