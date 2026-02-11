use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub cancel_flag: Arc<AtomicBool>,
}
