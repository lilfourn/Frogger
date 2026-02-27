mod commands;
mod data;
mod error;
mod models;
pub(crate) mod scope_path;
mod services;
mod shell;
mod state;

use commands::{
    chat_commands, file_commands, indexing_commands, search_commands, settings_commands,
};
use data::migrations;
use state::{AppState, IndexingProgressState};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64};
use std::sync::{Arc, Mutex};

fn register_sqlite_extensions() {
    data::register_sqlite_vec_extension();
}

fn init_runtime() {
    services::embedding_service::init_embedding_runtime();
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
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    migrations::run_migrations(&conn)?;
    Ok((conn, db_path))
}

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _guard = std::env::var("SENTRY_DSN")
        .ok()
        .filter(|dsn| !dsn.trim().is_empty())
        .map(|dsn| {
            sentry::init((
                dsn,
                sentry::ClientOptions {
                    release: sentry::release_name!(),
                    send_default_pii: false,
                    ..Default::default()
                },
            ))
        });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            init_runtime();
            let (conn, db_path) = init_db(app)?;
            app.manage(AppState {
                db: Mutex::new(conn),
                db_path: db_path.clone(),
                cancel_flag: Arc::new(AtomicBool::new(false)),
                file_operation_cancel_flags: Mutex::new(HashMap::new()),
                organize_cancel_flags: Mutex::new(HashMap::new()),
                organize_status: Mutex::new(HashMap::new()),
                organize_progress_sequence: std::sync::atomic::AtomicU64::new(0),
                watcher_handle: Mutex::new(None),
                indexing_status: Arc::new(Mutex::new(IndexingProgressState {
                    processed: 0,
                    total: 0,
                    status: "done".to_string(),
                })),
                last_user_interaction_at: Arc::new(AtomicI64::new(0)),
                permission_policy_cache: std::sync::RwLock::new(None),
                permission_policy_version: std::sync::atomic::AtomicU64::new(1),
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
            file_commands::read_file_text,
            file_commands::find_large_files,
            file_commands::find_old_files,
            file_commands::find_duplicates,
            file_commands::detect_project_type,
            file_commands::open_file,
            indexing_commands::start_indexing,
            indexing_commands::notify_user_interaction,
            indexing_commands::get_indexing_status,
            indexing_commands::stop_indexing,
            indexing_commands::clear_indexed_data,
            search_commands::search,
            settings_commands::save_api_key,
            settings_commands::has_api_key,
            settings_commands::delete_api_key,
            settings_commands::get_setting,
            settings_commands::set_setting,
            settings_commands::get_permission_scopes,
            settings_commands::check_permission_request,
            settings_commands::upsert_permission_scope,
            settings_commands::get_permission_defaults,
            settings_commands::set_permission_defaults,
            settings_commands::delete_permission_scope,
            settings_commands::normalize_permission_scopes,
            settings_commands::resolve_permission_grant_targets,
            settings_commands::get_audit_log,
            settings_commands::reembed_indexed_files,
            settings_commands::start_reembed_indexed_files,
            settings_commands::get_reembed_status,
            chat_commands::send_chat,
            chat_commands::send_organize_plan,
            chat_commands::send_organize_execute,
            chat_commands::send_organize_apply,
            chat_commands::get_organize_status,
            chat_commands::cancel_organize,
            chat_commands::get_chat_history,
            chat_commands::clear_chat_history,
            chat_commands::new_chat_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
