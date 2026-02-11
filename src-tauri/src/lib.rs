mod commands;
mod data;
mod error;
mod models;
mod services;
mod shell;
mod state;

use commands::file_commands;
use data::migrations;
use state::AppState;
use std::sync::Mutex;

fn init_db(app: &tauri::App) -> Result<rusqlite::Connection, Box<dyn std::error::Error>> {
    let app_dir = app
        .path()
        .app_data_dir()
        .expect("failed to resolve app data dir");
    std::fs::create_dir_all(&app_dir)?;
    let db_path = app_dir.join("frogger.db");
    let conn = rusqlite::Connection::open(db_path)?;
    migrations::run_migrations(&conn)?;
    Ok(conn)
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
            let conn = init_db(app)?;
            app.manage(AppState {
                db: Mutex::new(conn),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            file_commands::list_directory,
            file_commands::get_home_dir,
            file_commands::get_mounted_volumes,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
