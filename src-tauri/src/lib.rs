mod commands;
mod data;
mod error;
mod models;
mod services;
mod shell;
mod state;

use commands::{file_commands, indexing_commands, search_commands};
use data::migrations;
use state::AppState;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

fn register_sqlite_extensions() {
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }
}

fn init_db(
    app: &tauri::App,
) -> Result<(rusqlite::Connection, std::path::PathBuf), Box<dyn std::error::Error>> {
    register_sqlite_extensions();
    let app_dir = app
        .path()
        .app_data_dir()
        .expect("failed to resolve app data dir");
    std::fs::create_dir_all(&app_dir)?;
    let db_path = app_dir.join("frogger.db");
    let conn = rusqlite::Connection::open(&db_path)?;
    migrations::run_migrations(&conn)?;
    Ok((conn, db_path))
}

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let (conn, db_path) = init_db(app)?;
            app.manage(AppState {
                db: Mutex::new(conn),
                db_path,
                cancel_flag: Arc::new(AtomicBool::new(false)),
                watcher_handle: Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            file_commands::list_directory,
            file_commands::get_home_dir,
            file_commands::get_mounted_volumes,
            file_commands::create_directory,
            file_commands::rename_file,
            file_commands::move_files,
            file_commands::copy_files,
            file_commands::delete_files,
            file_commands::copy_files_with_progress,
            file_commands::cancel_operation,
            file_commands::undo_operation,
            file_commands::redo_operation,
            indexing_commands::start_indexing,
            indexing_commands::stop_indexing,
            search_commands::search,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
