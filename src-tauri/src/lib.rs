pub mod commands;
pub mod errors;
pub mod indexing;
pub mod models;
pub mod persistence;

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let database_path = app.path().app_data_dir()?.join("frogger.sqlite3");
            persistence::open_database(&database_path).map_err(|error| {
                let message = format!(
                    "failed to initialize Frogger database at {}: {error}",
                    database_path.display()
                );
                std::io::Error::other(message)
            })?;

            let app_handle = app.handle().clone();
            for window in commands::restored_windows_for_app(&app_handle).map_err(|error| {
                std::io::Error::other(format!("failed to restore Frogger windows: {error}"))
            })? {
                if window.label == "main" || app.get_webview_window(&window.label).is_some() {
                    continue;
                }

                let mut builder = WebviewWindowBuilder::new(
                    app,
                    window.label.clone(),
                    WebviewUrl::App("index.html".into()),
                )
                .title("Frogger")
                .title_bar_style(tauri::TitleBarStyle::Overlay)
                .hidden_title(true)
                .inner_size(window.geometry.width, window.geometry.height);

                builder = match (window.geometry.x, window.geometry.y) {
                    (Some(x), Some(y)) => builder.position(x, y),
                    _ => builder.center(),
                };

                builder.build()?;
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap_app,
            commands::cleanup_thumbnail_cache,
            commands::create_file_manager_window,
            commands::get_folder_view_state,
            commands::get_sidebar_state,
            commands::get_thumbnail,
            commands::list_directory,
            commands::open_file_with_default_app,
            commands::pin_sidebar_folder,
            commands::record_recent_item,
            commands::save_folder_view_state,
            commands::save_session_state,
            commands::search_metadata,
            commands::set_browser_display_setting,
            commands::set_sidebar_section_visibility,
            commands::unpin_sidebar_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
