use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::Ordering;
use tauri::{command, AppHandle, Emitter, State};

use crate::error::AppError;
use crate::models::file_entry::FileEntry;
use crate::models::operation::OperationType;
use crate::models::volume::VolumeInfo;
use crate::services::permission_service::{self, PermissionCapability};
use crate::services::{file_service, undo_service};
use crate::shell::safety::validate_path;
use crate::state::AppState;
use sha2::{Digest, Sha256};

fn lock_db_or_recover(state: &AppState) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
    state
        .db
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn record_operation_or_rollback<F, R>(
    state: &AppState,
    operation_name: &str,
    record: F,
    rollback: R,
) -> Result<(), AppError>
where
    F: FnOnce(&rusqlite::Connection) -> Result<(), AppError>,
    R: FnOnce() -> Result<(), AppError>,
{
    let record_result = {
        let conn = lock_db_or_recover(state);
        record(&conn)
    };

    if let Err(record_err) = record_result {
        if let Err(rollback_err) = rollback() {
            return Err(AppError::General(format!(
                "{operation_name} succeeded, undo logging failed ({record_err}), and rollback failed ({rollback_err})"
            )));
        }
        return Err(AppError::General(format!(
            "{operation_name} succeeded but undo logging failed; operation was rolled back: {record_err}"
        )));
    }

    Ok(())
}

fn rollback_copied_paths(paths: &[String]) -> Result<(), AppError> {
    for path in paths {
        let p = Path::new(path);
        if !p.exists() {
            continue;
        }
        if p.is_dir() {
            std::fs::remove_dir_all(p)?;
        } else {
            std::fs::remove_file(p)?;
        }
    }
    Ok(())
}

fn rollback_moved_paths(sources: &[String], dest_paths: &[String]) -> Result<(), AppError> {
    for (source, dest) in sources.iter().zip(dest_paths.iter()) {
        let src_parent = Path::new(source)
            .parent()
            .ok_or_else(|| AppError::General(format!("invalid source path: {source}")))?
            .to_string_lossy()
            .to_string();
        if !Path::new(dest).exists() {
            continue;
        }
        file_service::move_files(std::slice::from_ref(dest), &src_parent)?;
    }
    Ok(())
}

fn rollback_deleted_paths(results: &[file_service::DeleteResult]) -> Result<(), AppError> {
    for result in results {
        file_service::restore_from_trash(&result.trash_path, &result.original_path)?;
    }
    Ok(())
}

fn read_utf8_prefix(path: &Path, max_bytes: usize) -> Result<String, AppError> {
    let mut buf = vec![0u8; max_bytes];
    let mut file = fs::File::open(path)?;
    file.read_exact(&mut buf)?;

    match String::from_utf8(buf) {
        Ok(text) => Ok(text),
        Err(err) => {
            let bytes = err.into_bytes();
            let valid_up_to = std::str::from_utf8(&bytes)
                .map(|_| bytes.len())
                .unwrap_or_else(|utf8_err| utf8_err.valid_up_to());
            if valid_up_to == 0 {
                return Err(AppError::General("file is not valid UTF-8".to_string()));
            }

            String::from_utf8(bytes[..valid_up_to].to_vec())
                .map_err(|_| AppError::General("file is not valid UTF-8".to_string()))
        }
    }
}

fn hash_file_sha256(path: &Path) -> Result<String, AppError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let count = file.read(&mut buf)?;
        if count == 0 {
            break;
        }
        hasher.update(&buf[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[command]
pub fn list_directory(
    path: String,
    allow_once: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Vec<FileEntry>, AppError> {
    validate_path(&path)?;

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
    {
        let conn = lock_db_or_recover(state.inner());
        permission_service::enforce_cached(
            &conn,
            &state,
            &path,
            PermissionCapability::Modification,
            allow_once,
        )?;
    }

    let existed_before = Path::new(&path).exists();
    file_service::create_dir(&path)?;
    let meta = serde_json::json!({ "path": path });
    let affected_paths = vec![path.clone()];

    record_operation_or_rollback(
        state.inner(),
        "create_directory",
        |conn| {
            undo_service::record_operation(
                conn,
                OperationType::CreateDir,
                "create_dir",
                "remove_dir",
                &affected_paths,
                Some(&meta.to_string()),
            )
        },
        || {
            if !existed_before && Path::new(&path).is_dir() {
                std::fs::remove_dir(&path)?;
            }
            Ok(())
        },
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
        let conn = lock_db_or_recover(state.inner());
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
    let meta = serde_json::json!({ "source": source, "destination": destination });
    let affected_paths = vec![source.clone()];

    record_operation_or_rollback(
        state.inner(),
        "rename_file",
        |conn| {
            undo_service::record_operation(
                conn,
                OperationType::Rename,
                "rename",
                "rename_inverse",
                &affected_paths,
                Some(&meta.to_string()),
            )
        },
        || {
            if Path::new(&destination).exists() && !Path::new(&source).exists() {
                file_service::rename(&destination, &source)?;
            }
            Ok(())
        },
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
        let conn = lock_db_or_recover(state.inner());
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
    let moves: Vec<(&str, &str)> = dest_paths
        .iter()
        .zip(sources.iter())
        .map(|(d, s)| (d.as_str(), s.as_str()))
        .collect();
    let meta = serde_json::json!({ "moves": moves });

    record_operation_or_rollback(
        state.inner(),
        "move_files",
        |conn| {
            undo_service::record_operation(
                conn,
                OperationType::Move,
                "move",
                "move_inverse",
                &sources,
                Some(&meta.to_string()),
            )
        },
        || rollback_moved_paths(&sources, &dest_paths),
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
        let conn = lock_db_or_recover(state.inner());
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
    let meta = serde_json::json!({ "copied_paths": dest_paths.clone() });

    record_operation_or_rollback(
        state.inner(),
        "copy_files",
        |conn| {
            undo_service::record_operation(
                conn,
                OperationType::Copy,
                "copy",
                "remove_copies",
                &sources,
                Some(&meta.to_string()),
            )
        },
        || rollback_copied_paths(&dest_paths),
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
        let conn = lock_db_or_recover(state.inner());
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

    record_operation_or_rollback(
        state.inner(),
        "delete_files",
        |conn| {
            undo_service::record_operation(
                conn,
                OperationType::Delete,
                "delete",
                "restore",
                &paths,
                Some(&meta.to_string()),
            )
        },
        || rollback_deleted_paths(&results),
    )?;

    Ok(())
}

#[command]
pub fn copy_files_with_progress(
    sources: Vec<String>,
    dest_dir: String,
    operation_id: Option<String>,
    allow_once: Option<bool>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<String>, AppError> {
    let allow_once = allow_once.unwrap_or(false);
    {
        let conn = lock_db_or_recover(state.inner());
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

    let operation_id = operation_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let cancel = state.reset_file_operation_cancel_flag(&operation_id);

    let dest_paths = file_service::copy_files_with_progress(&sources, &dest_dir, &cancel, |evt| {
        let _ = app.emit("file-operation-progress", evt);
    });
    state.clear_file_operation_cancel_flag(&operation_id);

    let dest_paths = dest_paths?;

    let meta = serde_json::json!({ "copied_paths": dest_paths.clone() });

    record_operation_or_rollback(
        state.inner(),
        "copy_files_with_progress",
        |conn| {
            undo_service::record_operation(
                conn,
                OperationType::Copy,
                "copy",
                "remove_copies",
                &sources,
                Some(&meta.to_string()),
            )
        },
        || rollback_copied_paths(&dest_paths),
    )?;

    Ok(dest_paths)
}

#[command]
pub fn cancel_operation(operation_id: Option<String>, state: State<'_, AppState>) {
    state.mark_file_operation_cancelled(operation_id.as_deref());
    if operation_id.is_none() {
        state.cancel_flag.store(true, Ordering::Relaxed);
    }
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
    validate_path(&directory)?;

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
         FROM files WHERE (parent_path = ?1 OR parent_path LIKE ?1 || '/%') AND is_directory = 0 AND size_bytes >= ?2
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
    validate_path(&directory)?;

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
         FROM files WHERE (parent_path = ?1 OR parent_path LIKE ?1 || '/%') AND is_directory = 0 AND modified_at < ?2
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
    validate_path(&directory)?;

    let allow_once = allow_once.unwrap_or(false);
    let flat: Vec<FileEntry> = {
        let conn = lock_db_or_recover(state.inner());
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
               WHERE (parent_path = ?1 OR parent_path LIKE ?1 || '/%') AND is_directory = 0 AND size_bytes > 0
               GROUP BY size_bytes HAVING COUNT(*) > 1
             ) dups ON f.size_bytes = dups.size_bytes
             WHERE (f.parent_path = ?1 OR f.parent_path LIKE ?1 || '/%') AND f.is_directory = 0
             ORDER BY f.size_bytes DESC, f.name ASC",
        )?;
        let results: Vec<FileEntry> = stmt
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
        results
    };

    let mut groups: std::collections::HashMap<(i64, String), Vec<FileEntry>> =
        std::collections::HashMap::new();
    for entry in flat {
        let Some(size) = entry.size_bytes else {
            continue;
        };
        let path = Path::new(&entry.path);
        if !path.is_file() {
            continue;
        }
        let Ok(hash) = hash_file_sha256(path) else {
            continue;
        };
        groups.entry((size, hash)).or_default().push(entry);
    }

    let result: Vec<Vec<FileEntry>> = groups
        .into_values()
        .filter(|group| group.len() > 1)
        .collect();
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
    validate_path(&directory)?;

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

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.ends_with(".csproj") {
                return Ok(Some(ProjectInfo {
                    project_type: "dotnet".to_string(),
                    marker_file: file_name,
                    directory: directory.clone(),
                }));
            }
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
        let conn = lock_db_or_recover(state.inner());
        permission_service::enforce_cached(
            &conn,
            &state,
            &path,
            PermissionCapability::ContentScan,
            allow_once,
        )?;
    }

    validate_path(&path)?;

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
    validate_path(&path)?;

    let allow_once = allow_once.unwrap_or(false);
    {
        let conn = lock_db_or_recover(state.inner());
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
        return read_utf8_prefix(p, MAX_PREVIEW_BYTES as usize);
    }
    fs::read_to_string(p).map_err(|e| AppError::General(format!("failed to read file: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::migrations;
    use crate::data::repository;
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
