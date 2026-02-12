use crate::data::repository;
use crate::error::AppError;
use crate::models::operation::{OperationRecord, OperationType};
use crate::services::file_service;
use crate::services::permission_service::{self, PermissionCapability};
use rusqlite::Connection;

pub fn record_operation(
    conn: &Connection,
    op_type: OperationType,
    forward_cmd: &str,
    inverse_cmd: &str,
    affected_paths: &[String],
    metadata: Option<&str>,
) -> Result<(), AppError> {
    let record = OperationRecord {
        operation_id: uuid::Uuid::new_v4().to_string(),
        operation_type: op_type,
        forward_command: forward_cmd.to_string(),
        inverse_command: inverse_cmd.to_string(),
        affected_paths: affected_paths.to_vec(),
        metadata: metadata.map(serde_json::from_str).transpose()?,
        executed_at: chrono::Utc::now().to_rfc3339(),
        undone: false,
    };
    repository::insert_operation(conn, &record)?;
    Ok(())
}

pub fn undo(conn: &Connection, allow_once: bool) -> Result<String, AppError> {
    let op = repository::get_latest_undoable(conn)?
        .ok_or_else(|| AppError::General("nothing to undo".to_string()))?;

    execute_inverse(conn, &op, allow_once)?;
    repository::mark_undone(conn, &op.operation_id)?;

    Ok(format!("undone: {} {}", op.operation_type, op.operation_id))
}

pub fn redo(conn: &Connection, allow_once: bool) -> Result<String, AppError> {
    let op = repository::get_latest_redoable(conn)?
        .ok_or_else(|| AppError::General("nothing to redo".to_string()))?;

    execute_forward(conn, &op, allow_once)?;
    repository::mark_not_undone(conn, &op.operation_id)?;

    Ok(format!("redone: {} {}", op.operation_type, op.operation_id))
}

fn execute_inverse(
    conn: &Connection,
    op: &OperationRecord,
    allow_once: bool,
) -> Result<(), AppError> {
    let meta = op.metadata.clone().unwrap_or(serde_json::json!({}));

    match op.operation_type {
        OperationType::Rename => {
            let src = meta["destination"].as_str().unwrap_or("");
            let dest = meta["source"].as_str().unwrap_or("");
            permission_service::enforce(conn, src, PermissionCapability::Modification, allow_once)?;
            permission_service::enforce(
                conn,
                dest,
                PermissionCapability::Modification,
                allow_once,
            )?;
            file_service::rename(src, dest)?;
        }
        OperationType::Move => {
            let moves: Vec<(String, String)> =
                serde_json::from_value(meta["moves"].clone()).unwrap_or_default();
            for (dest, src) in moves {
                let dest_path = std::path::Path::new(&dest);
                let src_parent = std::path::Path::new(&src)
                    .parent()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                permission_service::enforce(
                    conn,
                    &dest,
                    PermissionCapability::Modification,
                    allow_once,
                )?;
                permission_service::enforce(
                    conn,
                    &src_parent,
                    PermissionCapability::Modification,
                    allow_once,
                )?;
                file_service::move_files(&[dest_path.to_string_lossy().to_string()], &src_parent)?;
            }
        }
        OperationType::Copy => {
            let copied: Vec<String> =
                serde_json::from_value(meta["copied_paths"].clone()).unwrap_or_default();
            for path in copied {
                permission_service::enforce(
                    conn,
                    &path,
                    PermissionCapability::Modification,
                    allow_once,
                )?;
                let p = std::path::Path::new(&path);
                if p.is_dir() {
                    std::fs::remove_dir_all(p)?;
                } else if p.exists() {
                    std::fs::remove_file(p)?;
                }
            }
        }
        OperationType::Delete => {
            let items: Vec<serde_json::Value> =
                serde_json::from_value(meta["deleted_items"].clone()).unwrap_or_default();
            for item in items {
                let trash = item["trash_path"].as_str().unwrap_or("");
                let original = item["original_path"].as_str().unwrap_or("");
                permission_service::enforce(
                    conn,
                    original,
                    PermissionCapability::Modification,
                    allow_once,
                )?;
                file_service::restore_from_trash(trash, original)?;
            }
        }
        OperationType::CreateDir => {
            let path = meta["path"].as_str().unwrap_or("");
            permission_service::enforce(
                conn,
                path,
                PermissionCapability::Modification,
                allow_once,
            )?;
            if std::path::Path::new(path).is_dir() {
                std::fs::remove_dir(path)?;
            }
        }
        OperationType::BatchRename => {
            let renames: Vec<(String, String)> =
                serde_json::from_value(meta["renames"].clone()).unwrap_or_default();
            for (new_name, old_name) in renames {
                permission_service::enforce(
                    conn,
                    &new_name,
                    PermissionCapability::Modification,
                    allow_once,
                )?;
                permission_service::enforce(
                    conn,
                    &old_name,
                    PermissionCapability::Modification,
                    allow_once,
                )?;
                file_service::rename(&new_name, &old_name)?;
            }
        }
    }
    Ok(())
}

fn execute_forward(
    conn: &Connection,
    op: &OperationRecord,
    allow_once: bool,
) -> Result<(), AppError> {
    let meta = op.metadata.clone().unwrap_or(serde_json::json!({}));

    match op.operation_type {
        OperationType::Rename => {
            let src = meta["source"].as_str().unwrap_or("");
            let dest = meta["destination"].as_str().unwrap_or("");
            permission_service::enforce(conn, src, PermissionCapability::Modification, allow_once)?;
            permission_service::enforce(
                conn,
                dest,
                PermissionCapability::Modification,
                allow_once,
            )?;
            file_service::rename(src, dest)?;
        }
        OperationType::CreateDir => {
            let path = meta["path"].as_str().unwrap_or("");
            permission_service::enforce(
                conn,
                path,
                PermissionCapability::Modification,
                allow_once,
            )?;
            file_service::create_dir(path)?;
        }
        _ => {
            return Err(AppError::General(format!(
                "redo not supported for {}",
                op.operation_type
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::migrations;
    use std::fs::{self, File};
    use std::io::Write;

    fn setup_db() -> Connection {
        let dir = std::env::temp_dir().join(format!("frogger_undo_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let conn = Connection::open(dir.join("test.db")).unwrap();
        migrations::run_migrations(&conn).unwrap();
        repository::set_setting(&conn, "permission_default_content_scan", "allow").unwrap();
        repository::set_setting(&conn, "permission_default_modification", "allow").unwrap();
        repository::set_setting(&conn, "permission_default_ocr", "allow").unwrap();
        repository::set_setting(&conn, "permission_default_indexing", "allow").unwrap();
        conn
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("frogger_undo_{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_undo_rename() {
        let conn = setup_db();
        let base = temp_dir("undo_rename");
        let src = base.join("original.txt");
        let dest = base.join("renamed.txt");
        File::create(&src).unwrap().write_all(b"test").unwrap();

        file_service::rename(&src.to_string_lossy(), &dest.to_string_lossy()).unwrap();

        let meta = serde_json::json!({
            "source": src.to_string_lossy(),
            "destination": dest.to_string_lossy(),
        });
        record_operation(
            &conn,
            OperationType::Rename,
            "rename",
            "rename_inverse",
            &[src.to_string_lossy().to_string()],
            Some(&meta.to_string()),
        )
        .unwrap();

        assert!(!src.exists());
        assert!(dest.exists());

        undo(&conn, false).unwrap();

        assert!(src.exists());
        assert!(!dest.exists());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_undo_delete() {
        let conn = setup_db();
        let base = temp_dir("undo_delete");
        let file = base.join("delete_me.txt");
        File::create(&file).unwrap().write_all(b"precious").unwrap();
        let file_str = file.to_string_lossy().to_string();

        let results = file_service::soft_delete(&[file_str.clone()]).unwrap();

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

        record_operation(
            &conn,
            OperationType::Delete,
            "delete",
            "restore",
            &[file_str.clone()],
            Some(&meta.to_string()),
        )
        .unwrap();

        assert!(!file.exists());

        undo(&conn, false).unwrap();

        assert!(file.exists());
        assert_eq!(fs::read_to_string(&file).unwrap(), "precious");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_redo_rename() {
        let conn = setup_db();
        let base = temp_dir("redo_rename");
        let src = base.join("orig.txt");
        let dest = base.join("new.txt");
        File::create(&src).unwrap().write_all(b"x").unwrap();

        file_service::rename(&src.to_string_lossy(), &dest.to_string_lossy()).unwrap();

        let meta = serde_json::json!({
            "source": src.to_string_lossy(),
            "destination": dest.to_string_lossy(),
        });
        record_operation(
            &conn,
            OperationType::Rename,
            "rename",
            "rename_inverse",
            &[src.to_string_lossy().to_string()],
            Some(&meta.to_string()),
        )
        .unwrap();

        undo(&conn, false).unwrap();
        assert!(src.exists());
        assert!(!dest.exists());

        redo(&conn, false).unwrap();
        assert!(!src.exists());
        assert!(dest.exists());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_undo_empty_returns_error() {
        let conn = setup_db();
        assert!(undo(&conn, false).is_err());
    }

    #[test]
    fn test_redo_empty_returns_error() {
        let conn = setup_db();
        assert!(redo(&conn, false).is_err());
    }
}
