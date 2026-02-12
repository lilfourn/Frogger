use std::fs;
use std::path::Path;
use std::sync::atomic::Ordering;
use tauri::{command, AppHandle, Emitter, State};

use crate::data::repository;
use crate::error::AppError;
use crate::models::file_entry::FileEntry;
use crate::models::operation::OperationType;
use crate::models::volume::VolumeInfo;
use crate::services::{file_service, undo_service};
use crate::state::AppState;

#[command]
pub fn list_directory(
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<FileEntry>, AppError> {
    let entries = read_directory(&path)?;

    if let Ok(conn) = state.db.try_lock() {
        for entry in &entries {
            let modified = entry.modified_at.as_deref().unwrap_or("");
            if repository::needs_reindex(&conn, &entry.path, modified) {
                let _ = repository::insert_file(&conn, entry);
                let _ = repository::insert_fts(&conn, &entry.path, &entry.name, "");
            }
        }
    }

    Ok(entries)
}

fn read_directory(path: &str) -> Result<Vec<FileEntry>, AppError> {
    let dir_path = Path::new(path);
    if !dir_path.is_dir() {
        return Err(AppError::General(format!("not a directory: {path}")));
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        let file_path = entry.path().to_string_lossy().to_string();

        let extension = Path::new(&file_name)
            .extension()
            .map(|e| e.to_string_lossy().to_string());

        let mime_type = extension
            .as_ref()
            .and_then(|ext| mime_guess::from_ext(ext).first())
            .map(|m| m.to_string());

        let parent = dir_path.to_string_lossy().to_string();

        entries.push(FileEntry {
            path: file_path,
            name: file_name,
            extension,
            mime_type,
            size_bytes: Some(metadata.len() as i64),
            created_at: metadata
                .created()
                .ok()
                .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339()),
            modified_at: metadata
                .modified()
                .ok()
                .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339()),
            is_directory: metadata.is_dir(),
            parent_path: Some(parent),
        });
    }

    entries.sort_by(|a, b| {
        b.is_directory
            .cmp(&a.is_directory)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}

#[command]
pub fn get_home_dir() -> Result<String, AppError> {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| AppError::General("could not resolve home directory".to_string()))
}

#[command]
pub fn get_mounted_volumes() -> Result<Vec<VolumeInfo>, AppError> {
    let mut volumes = Vec::new();

    #[cfg(target_os = "macos")]
    {
        volumes.push(VolumeInfo {
            name: "Macintosh HD".to_string(),
            path: "/".to_string(),
            total_bytes: None,
            free_bytes: None,
        });

        let volumes_dir = Path::new("/Volumes");
        if volumes_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(volumes_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name != "Macintosh HD" {
                            volumes.push(VolumeInfo {
                                name,
                                path: path.to_string_lossy().to_string(),
                                total_bytes: None,
                                free_bytes: None,
                            });
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        volumes.push(VolumeInfo {
            name: "/".to_string(),
            path: "/".to_string(),
            total_bytes: None,
            free_bytes: None,
        });
    }

    #[cfg(target_os = "windows")]
    {
        for letter in b'A'..=b'Z' {
            let drive = format!("{}:\\", letter as char);
            if Path::new(&drive).exists() {
                volumes.push(VolumeInfo {
                    name: format!("{}: Drive", letter as char),
                    path: drive,
                    total_bytes: None,
                    free_bytes: None,
                });
            }
        }
    }

    Ok(volumes)
}

#[command]
pub fn create_directory(path: String, state: State<'_, AppState>) -> Result<(), AppError> {
    file_service::create_dir(&path)?;
    let conn = state.db.lock().unwrap();
    let meta = serde_json::json!({ "path": path });
    undo_service::record_operation(
        &conn,
        OperationType::CreateDir,
        "create_dir",
        "remove_dir",
        &[path],
        Some(&meta.to_string()),
    )?;
    Ok(())
}

#[command]
pub fn rename_file(
    source: String,
    destination: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    file_service::rename(&source, &destination)?;
    let conn = state.db.lock().unwrap();
    let meta = serde_json::json!({ "source": source, "destination": destination });
    undo_service::record_operation(
        &conn,
        OperationType::Rename,
        "rename",
        "rename_inverse",
        &[source],
        Some(&meta.to_string()),
    )?;
    Ok(())
}

#[command]
pub fn move_files(
    sources: Vec<String>,
    dest_dir: String,
    state: State<'_, AppState>,
) -> Result<Vec<String>, AppError> {
    let dest_paths = file_service::move_files(&sources, &dest_dir)?;
    let conn = state.db.lock().unwrap();
    let moves: Vec<(&str, &str)> = dest_paths
        .iter()
        .zip(sources.iter())
        .map(|(d, s)| (d.as_str(), s.as_str()))
        .collect();
    let meta = serde_json::json!({ "moves": moves });
    undo_service::record_operation(
        &conn,
        OperationType::Move,
        "move",
        "move_inverse",
        &sources,
        Some(&meta.to_string()),
    )?;
    Ok(dest_paths)
}

#[command]
pub fn copy_files(
    sources: Vec<String>,
    dest_dir: String,
    state: State<'_, AppState>,
) -> Result<Vec<String>, AppError> {
    let dest_paths = file_service::copy_files(&sources, &dest_dir)?;
    let conn = state.db.lock().unwrap();
    let meta = serde_json::json!({ "copied_paths": dest_paths });
    undo_service::record_operation(
        &conn,
        OperationType::Copy,
        "copy",
        "remove_copies",
        &sources,
        Some(&meta.to_string()),
    )?;
    Ok(dest_paths)
}

#[command]
pub fn delete_files(paths: Vec<String>, state: State<'_, AppState>) -> Result<(), AppError> {
    let results = file_service::soft_delete(&paths)?;
    let conn = state.db.lock().unwrap();
    let items: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "trash_path": r.trash_path,
                "original_path": r.original_path,
            })
        })
        .collect();
    let meta = serde_json::json!({ "deleted_items": items });
    undo_service::record_operation(
        &conn,
        OperationType::Delete,
        "delete",
        "restore",
        &paths,
        Some(&meta.to_string()),
    )?;
    Ok(())
}

#[command]
pub fn copy_files_with_progress(
    sources: Vec<String>,
    dest_dir: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<String>, AppError> {
    state.cancel_flag.store(false, Ordering::Relaxed);
    let cancel = state.cancel_flag.clone();

    let dest_paths = file_service::copy_files_with_progress(&sources, &dest_dir, &cancel, |evt| {
        let _ = app.emit("file-operation-progress", evt);
    })?;

    let conn = state.db.lock().unwrap();
    let meta = serde_json::json!({ "copied_paths": dest_paths });
    undo_service::record_operation(
        &conn,
        OperationType::Copy,
        "copy",
        "remove_copies",
        &sources,
        Some(&meta.to_string()),
    )?;
    Ok(dest_paths)
}

#[command]
pub fn cancel_operation(state: State<'_, AppState>) {
    state.cancel_flag.store(true, Ordering::Relaxed);
}

#[command]
pub fn undo_operation(state: State<'_, AppState>) -> Result<String, AppError> {
    let conn = state.db.lock().unwrap();
    undo_service::undo(&conn)
}

#[command]
pub fn redo_operation(state: State<'_, AppState>) -> Result<String, AppError> {
    let conn = state.db.lock().unwrap();
    undo_service::redo(&conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::migrations;
    use std::fs::File;

    #[test]
    fn test_list_directory_returns_entries() {
        let dir = std::env::temp_dir().join("frogger_test_list_dir");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        File::create(dir.join("file_a.txt")).unwrap();
        File::create(dir.join("file_b.md")).unwrap();
        fs::create_dir_all(dir.join("subdir")).unwrap();

        let result = read_directory(&dir.to_string_lossy()).unwrap();

        assert_eq!(result.len(), 3);
        assert!(result[0].is_directory);
        assert_eq!(result[0].name, "subdir");

        let names: Vec<&str> = result.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"file_a.txt"));
        assert!(names.contains(&"file_b.md"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_list_directory_invalid_path() {
        let result = read_directory("/nonexistent/path/1234567890");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_home_dir() {
        let home = get_home_dir().unwrap();
        assert!(!home.is_empty());
        assert!(Path::new(&home).is_dir());
    }

    #[test]
    fn test_get_mounted_volumes() {
        let volumes = get_mounted_volumes().unwrap();
        assert!(!volumes.is_empty());
        assert!(volumes.iter().any(|v| v.path == "/"));
    }

    #[test]
    fn test_list_directory_populates_metadata() {
        let dir = std::env::temp_dir().join("frogger_test_list_meta");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("test.txt"), "hello world").unwrap();

        let result = read_directory(&dir.to_string_lossy()).unwrap();
        let file = &result[0];

        assert_eq!(file.name, "test.txt");
        assert_eq!(file.extension.as_deref(), Some("txt"));
        assert_eq!(file.mime_type.as_deref(), Some("text/plain"));
        assert_eq!(file.size_bytes, Some(11));
        assert!(!file.is_directory);
        assert!(file.created_at.is_some());
        assert!(file.modified_at.is_some());
        assert_eq!(
            file.parent_path.as_deref(),
            Some(dir.to_string_lossy().as_ref())
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_list_directory_indexes_to_db() {
        let dir = std::env::temp_dir().join("frogger_test_list_index");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        File::create(dir.join("indexed.txt")).unwrap();

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        migrations::run_migrations(&conn).unwrap();

        let entries = read_directory(&dir.to_string_lossy()).unwrap();
        for entry in &entries {
            let modified = entry.modified_at.as_deref().unwrap_or("");
            if repository::needs_reindex(&conn, &entry.path, modified) {
                repository::insert_file(&conn, entry).unwrap();
                repository::insert_fts(&conn, &entry.path, &entry.name, "").unwrap();
            }
        }

        let file_path = dir.join("indexed.txt").to_string_lossy().to_string();
        let stored = repository::get_by_path(&conn, &file_path).unwrap();
        assert!(stored.is_some());
        assert_eq!(stored.unwrap().name, "indexed.txt");

        let fts = repository::search_fts(&conn, "indexed", 10).unwrap();
        assert_eq!(fts.len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }
}
