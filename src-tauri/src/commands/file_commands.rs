use std::fs;
use std::path::Path;
use std::sync::atomic::Ordering;
use tauri::{command, AppHandle, Emitter, State};

use crate::data::repository;
use crate::error::AppError;
use crate::models::file_entry::FileEntry;
use crate::models::operation::OperationType;
use crate::models::volume::VolumeInfo;
use crate::services::permission_service::{self, PermissionCapability};
use crate::services::{file_service, undo_service};
use crate::state::AppState;

#[command]
pub fn list_directory(
    path: String,
    allow_once: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Vec<FileEntry>, AppError> {
    let allow_once = allow_once.unwrap_or(false);
    {
        let conn = state
            .db
            .lock()
            .map_err(|e| AppError::General(e.to_string()))?;
        permission_service::enforce_cached(
            &conn,
            &state,
            &path,
            PermissionCapability::ContentScan,
            allow_once,
        )?;
    }

    let entries = read_directory(&path)?;

    if let Ok(conn) = state.db.try_lock() {
        if let Ok(policy) = permission_service::load_policy_cache_entry(&conn, &state) {
            for entry in &entries {
                if permission_service::enforce_with_cached_policy(
                    &policy,
                    &entry.path,
                    PermissionCapability::Indexing,
                    false,
                )
                .is_err()
                {
                    continue;
                }
                let modified = entry.modified_at.as_deref().unwrap_or("");
                if repository::needs_reindex(&conn, &entry.path, modified) {
                    let _ = repository::insert_file(&conn, entry);
                    let _ = repository::insert_fts(&conn, &entry.path, &entry.name, "");
                }
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
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
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
pub fn create_directory(
    path: String,
    allow_once: Option<bool>,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let allow_once = allow_once.unwrap_or(false);
    let conn = state
        .db
        .lock()
        .map_err(|e| AppError::General(e.to_string()))?;
    permission_service::enforce_cached(
        &conn,
        &state,
        &path,
        PermissionCapability::Modification,
        allow_once,
    )?;
    drop(conn);

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
    allow_once: Option<bool>,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let allow_once = allow_once.unwrap_or(false);
    {
        let conn = state
            .db
            .lock()
            .map_err(|e| AppError::General(e.to_string()))?;
        permission_service::enforce_cached(
            &conn,
            &state,
            &source,
            PermissionCapability::Modification,
            allow_once,
        )?;
        permission_service::enforce_cached(
            &conn,
            &state,
            &destination,
            PermissionCapability::Modification,
            allow_once,
        )?;
    }

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
    allow_once: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Vec<String>, AppError> {
    let allow_once = allow_once.unwrap_or(false);
    {
        let conn = state
            .db
            .lock()
            .map_err(|e| AppError::General(e.to_string()))?;
        permission_service::enforce_cached(
            &conn,
            &state,
            &dest_dir,
            PermissionCapability::Modification,
            allow_once,
        )?;
        for src in &sources {
            permission_service::enforce_cached(
                &conn,
                &state,
                src,
                PermissionCapability::Modification,
                allow_once,
            )?;
        }
    }

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
    allow_once: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Vec<String>, AppError> {
    let allow_once = allow_once.unwrap_or(false);
    {
        let conn = state
            .db
            .lock()
            .map_err(|e| AppError::General(e.to_string()))?;
        permission_service::enforce_cached(
            &conn,
            &state,
            &dest_dir,
            PermissionCapability::Modification,
            allow_once,
        )?;
        for src in &sources {
            permission_service::enforce_cached(
                &conn,
                &state,
                src,
                PermissionCapability::Modification,
                allow_once,
            )?;
        }
    }

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
pub fn delete_files(
    paths: Vec<String>,
    allow_once: Option<bool>,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let allow_once = allow_once.unwrap_or(false);
    {
        let conn = state
            .db
            .lock()
            .map_err(|e| AppError::General(e.to_string()))?;
        for path in &paths {
            permission_service::enforce_cached(
                &conn,
                &state,
                path,
                PermissionCapability::Modification,
                allow_once,
            )?;
        }
    }

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
    allow_once: Option<bool>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<String>, AppError> {
    let allow_once = allow_once.unwrap_or(false);
    {
        let conn = state
            .db
            .lock()
            .map_err(|e| AppError::General(e.to_string()))?;
        permission_service::enforce_cached(
            &conn,
            &state,
            &dest_dir,
            PermissionCapability::Modification,
            allow_once,
        )?;
        for src in &sources {
            permission_service::enforce_cached(
                &conn,
                &state,
                src,
                PermissionCapability::Modification,
                allow_once,
            )?;
        }
    }

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
pub fn undo_operation(
    allow_once: Option<bool>,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let conn = state.db.lock().unwrap();
    undo_service::undo(&conn, allow_once.unwrap_or(false))
}

#[command]
pub fn redo_operation(
    allow_once: Option<bool>,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let conn = state.db.lock().unwrap();
    undo_service::redo(&conn, allow_once.unwrap_or(false))
}

#[command]
pub fn find_large_files(
    directory: String,
    min_size: i64,
    allow_once: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Vec<FileEntry>, AppError> {
    let allow_once = allow_once.unwrap_or(false);
    let conn = state
        .db
        .lock()
        .map_err(|e| AppError::General(e.to_string()))?;
    permission_service::enforce_cached(
        &conn,
        &state,
        &directory,
        PermissionCapability::ContentScan,
        allow_once,
    )?;
    let mut stmt = conn.prepare(
        "SELECT path, name, extension, mime_type, size_bytes, created_at, modified_at, is_directory, parent_path
         FROM files WHERE parent_path LIKE ?1 || '%' AND is_directory = 0 AND size_bytes >= ?2
         ORDER BY size_bytes DESC LIMIT 100",
    )?;
    let entries = stmt
        .query_map(rusqlite::params![directory, min_size], |row| {
            Ok(FileEntry {
                path: row.get(0)?,
                name: row.get(1)?,
                extension: row.get(2)?,
                mime_type: row.get(3)?,
                size_bytes: row.get(4)?,
                created_at: row.get(5)?,
                modified_at: row.get(6)?,
                is_directory: row.get(7)?,
                parent_path: row.get(8)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(entries)
}

#[command]
pub fn find_old_files(
    directory: String,
    older_than_days: i64,
    allow_once: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Vec<FileEntry>, AppError> {
    let allow_once = allow_once.unwrap_or(false);
    let cutoff = chrono::Utc::now() - chrono::Duration::days(older_than_days);
    let cutoff_str = cutoff.to_rfc3339();
    let conn = state
        .db
        .lock()
        .map_err(|e| AppError::General(e.to_string()))?;
    permission_service::enforce_cached(
        &conn,
        &state,
        &directory,
        PermissionCapability::ContentScan,
        allow_once,
    )?;
    let mut stmt = conn.prepare(
        "SELECT path, name, extension, mime_type, size_bytes, created_at, modified_at, is_directory, parent_path
         FROM files WHERE parent_path LIKE ?1 || '%' AND is_directory = 0 AND modified_at < ?2
         ORDER BY modified_at ASC LIMIT 100",
    )?;
    let entries = stmt
        .query_map(rusqlite::params![directory, cutoff_str], |row| {
            Ok(FileEntry {
                path: row.get(0)?,
                name: row.get(1)?,
                extension: row.get(2)?,
                mime_type: row.get(3)?,
                size_bytes: row.get(4)?,
                created_at: row.get(5)?,
                modified_at: row.get(6)?,
                is_directory: row.get(7)?,
                parent_path: row.get(8)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(entries)
}

#[command]
pub fn find_duplicates(
    directory: String,
    allow_once: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Vec<Vec<FileEntry>>, AppError> {
    let allow_once = allow_once.unwrap_or(false);
    let conn = state
        .db
        .lock()
        .map_err(|e| AppError::General(e.to_string()))?;
    permission_service::enforce_cached(
        &conn,
        &state,
        &directory,
        PermissionCapability::ContentScan,
        allow_once,
    )?;
    let mut stmt = conn.prepare(
        "SELECT f.path, f.name, f.extension, f.mime_type, f.size_bytes, f.created_at, f.modified_at, f.is_directory, f.parent_path
         FROM files f
         INNER JOIN (
           SELECT size_bytes FROM files
           WHERE parent_path LIKE ?1 || '%' AND is_directory = 0 AND size_bytes > 0
           GROUP BY size_bytes HAVING COUNT(*) > 1
         ) dups ON f.size_bytes = dups.size_bytes
         WHERE f.parent_path LIKE ?1 || '%' AND f.is_directory = 0
         ORDER BY f.size_bytes DESC, f.name ASC",
    )?;
    let flat: Vec<FileEntry> = stmt
        .query_map(rusqlite::params![directory], |row| {
            Ok(FileEntry {
                path: row.get(0)?,
                name: row.get(1)?,
                extension: row.get(2)?,
                mime_type: row.get(3)?,
                size_bytes: row.get(4)?,
                created_at: row.get(5)?,
                modified_at: row.get(6)?,
                is_directory: row.get(7)?,
                parent_path: row.get(8)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    let mut groups: std::collections::HashMap<i64, Vec<FileEntry>> =
        std::collections::HashMap::new();
    for entry in flat {
        if let Some(size) = entry.size_bytes {
            groups.entry(size).or_default().push(entry);
        }
    }
    let result: Vec<Vec<FileEntry>> = groups.into_values().filter(|g| g.len() > 1).collect();
    Ok(result)
}

#[derive(serde::Serialize)]
pub struct ProjectInfo {
    pub project_type: String,
    pub marker_file: String,
    pub directory: String,
}

#[command]
pub fn detect_project_type(
    directory: String,
    allow_once: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Option<ProjectInfo>, AppError> {
    let allow_once = allow_once.unwrap_or(false);
    {
        let conn = state
            .db
            .lock()
            .map_err(|e| AppError::General(e.to_string()))?;
        permission_service::enforce_cached(
            &conn,
            &state,
            &directory,
            PermissionCapability::ContentScan,
            allow_once,
        )?;
    }

    let dir = Path::new(&directory);
    let markers = [
        ("package.json", "node"),
        ("Cargo.toml", "rust"),
        ("pyproject.toml", "python"),
        ("setup.py", "python"),
        ("go.mod", "go"),
        ("pom.xml", "java"),
        ("build.gradle", "java"),
        ("Gemfile", "ruby"),
        ("composer.json", "php"),
        ("Package.swift", "swift"),
        (".csproj", "dotnet"),
    ];
    for (marker, project_type) in markers {
        if dir.join(marker).exists() {
            return Ok(Some(ProjectInfo {
                project_type: project_type.to_string(),
                marker_file: marker.to_string(),
                directory: directory.clone(),
            }));
        }
    }
    Ok(None)
}

#[command]
pub fn open_file(
    path: String,
    allow_once: Option<bool>,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let allow_once = allow_once.unwrap_or(false);
    {
        let conn = state
            .db
            .lock()
            .map_err(|e| AppError::General(e.to_string()))?;
        permission_service::enforce_cached(
            &conn,
            &state,
            &path,
            PermissionCapability::ContentScan,
            allow_once,
        )?;
    }

    let p = Path::new(&path);
    if !p.exists() {
        return Err(AppError::General(format!("path does not exist: {path}")));
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| AppError::General(format!("failed to open: {e}")))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| AppError::General(format!("failed to open: {e}")))?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &path])
            .spawn()
            .map_err(|e| AppError::General(format!("failed to open: {e}")))?;
    }
    Ok(())
}

const MAX_PREVIEW_BYTES: u64 = 1_048_576; // 1MB

#[command]
pub fn read_file_text(
    path: String,
    allow_once: Option<bool>,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let allow_once = allow_once.unwrap_or(false);
    {
        let conn = state
            .db
            .lock()
            .map_err(|e| AppError::General(e.to_string()))?;
        permission_service::enforce_cached(
            &conn,
            &state,
            &path,
            PermissionCapability::ContentScan,
            allow_once,
        )?;
    }

    let p = Path::new(&path);
    if !p.is_file() {
        return Err(AppError::General(format!("not a file: {path}")));
    }
    let meta = fs::metadata(p)?;
    if meta.len() > MAX_PREVIEW_BYTES {
        let mut buf = vec![0u8; MAX_PREVIEW_BYTES as usize];
        use std::io::Read;
        let mut f = fs::File::open(p)?;
        f.read_exact(&mut buf)?;
        return String::from_utf8(buf)
            .map_err(|_| AppError::General("file is not valid UTF-8".to_string()));
    }
    fs::read_to_string(p).map_err(|e| AppError::General(format!("failed to read file: {e}")))
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
    fn test_read_directory_skips_inaccessible_entries() {
        let dir = std::env::temp_dir().join("frogger_test_skip_inaccessible");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        File::create(dir.join("readable.txt")).unwrap();

        // Broken symlink — DirEntry::metadata() uses lstat so it succeeds,
        // but this verifies read_directory doesn't crash on unusual entries.
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.join("nonexistent_target"), dir.join("broken_link"))
            .unwrap();

        let result = read_directory(&dir.to_string_lossy()).unwrap();
        let names: Vec<&str> = result.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"readable.txt"));

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
