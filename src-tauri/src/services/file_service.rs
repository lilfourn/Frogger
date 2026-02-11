use crate::error::AppError;
use crate::shell::safety::{validate_not_protected, validate_path};
use std::fs;
use std::path::Path;

pub fn create_dir(path: &str) -> Result<(), AppError> {
    validate_path(path)?;
    validate_not_protected(path)?;
    fs::create_dir_all(path)?;
    Ok(())
}

pub fn rename(source: &str, destination: &str) -> Result<(), AppError> {
    validate_path(source)?;
    validate_path(destination)?;
    validate_not_protected(source)?;

    if !Path::new(source).exists() {
        return Err(AppError::General(format!(
            "source does not exist: {source}"
        )));
    }
    if Path::new(destination).exists() {
        return Err(AppError::General(format!(
            "destination already exists: {destination}"
        )));
    }

    fs::rename(source, destination)?;
    Ok(())
}

pub fn move_files(sources: &[String], dest_dir: &str) -> Result<Vec<String>, AppError> {
    validate_path(dest_dir)?;
    if !Path::new(dest_dir).is_dir() {
        return Err(AppError::General(format!(
            "destination is not a directory: {dest_dir}"
        )));
    }

    let mut dest_paths = Vec::new();
    for src in sources {
        validate_path(src)?;
        validate_not_protected(src)?;
        let src_path = Path::new(src);
        let file_name = src_path
            .file_name()
            .ok_or_else(|| AppError::General(format!("invalid source path: {src}")))?;
        let dest = Path::new(dest_dir).join(file_name);
        fs::rename(src_path, &dest)?;
        dest_paths.push(dest.to_string_lossy().to_string());
    }
    Ok(dest_paths)
}

pub fn copy_files(sources: &[String], dest_dir: &str) -> Result<Vec<String>, AppError> {
    validate_path(dest_dir)?;
    if !Path::new(dest_dir).is_dir() {
        return Err(AppError::General(format!(
            "destination is not a directory: {dest_dir}"
        )));
    }

    let mut dest_paths = Vec::new();
    for src in sources {
        validate_path(src)?;
        let src_path = Path::new(src);
        let file_name = src_path
            .file_name()
            .ok_or_else(|| AppError::General(format!("invalid source path: {src}")))?;
        let dest = Path::new(dest_dir).join(file_name);

        if src_path.is_dir() {
            copy_dir_recursive(src_path, &dest)?;
        } else {
            fs::copy(src_path, &dest)?;
        }
        dest_paths.push(dest.to_string_lossy().to_string());
    }
    Ok(dest_paths)
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), AppError> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let dest_child = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_child)?;
        } else {
            fs::copy(entry.path(), &dest_child)?;
        }
    }
    Ok(())
}

pub fn trash_dir() -> Result<std::path::PathBuf, AppError> {
    let home = dirs::home_dir()
        .ok_or_else(|| AppError::General("could not resolve home directory".to_string()))?;
    let trash = home.join(".frogger").join("trash");
    fs::create_dir_all(&trash)?;
    Ok(trash)
}

pub struct DeleteResult {
    pub trash_path: String,
    pub original_path: String,
}

pub fn soft_delete(paths: &[String]) -> Result<Vec<DeleteResult>, AppError> {
    let trash = trash_dir()?;
    let mut results = Vec::new();

    for src in paths {
        validate_path(src)?;
        validate_not_protected(src)?;

        let src_path = Path::new(src);
        if !src_path.exists() {
            return Err(AppError::General(format!("path does not exist: {src}")));
        }

        let id = uuid::Uuid::new_v4().to_string();
        let item_trash_dir = trash.join(&id);
        fs::create_dir_all(&item_trash_dir)?;

        let file_name = src_path
            .file_name()
            .ok_or_else(|| AppError::General(format!("invalid path: {src}")))?;
        let dest = item_trash_dir.join(file_name);

        fs::rename(src_path, &dest)?;

        let metadata = serde_json::json!({
            "original_path": src,
            "deleted_at": chrono::Utc::now().to_rfc3339(),
            "file_name": file_name.to_string_lossy(),
        });
        fs::write(
            item_trash_dir.join("metadata.json"),
            serde_json::to_string_pretty(&metadata)?,
        )?;

        results.push(DeleteResult {
            trash_path: dest.to_string_lossy().to_string(),
            original_path: src.clone(),
        });
    }
    Ok(results)
}

pub fn restore_from_trash(trash_path: &str, original_path: &str) -> Result<(), AppError> {
    let trash = Path::new(trash_path);
    let original = Path::new(original_path);

    if !trash.exists() {
        return Err(AppError::General(format!(
            "trash item does not exist: {trash_path}"
        )));
    }

    if let Some(parent) = original.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::rename(trash, original)?;

    if let Some(trash_parent) = trash.parent() {
        let _ = fs::remove_dir_all(trash_parent);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("frogger_test_{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_create_dir() {
        let base = temp_dir("create_dir");
        let target = base.join("new_folder");
        create_dir(&target.to_string_lossy()).unwrap();
        assert!(target.is_dir());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_rename() {
        let base = temp_dir("rename");
        let src = base.join("old.txt");
        let dest = base.join("new.txt");
        File::create(&src).unwrap().write_all(b"content").unwrap();

        rename(&src.to_string_lossy(), &dest.to_string_lossy()).unwrap();

        assert!(!src.exists());
        assert!(dest.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "content");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_move_files() {
        let base = temp_dir("move");
        let src = base.join("file.txt");
        let dest_dir = base.join("target");
        File::create(&src).unwrap().write_all(b"data").unwrap();
        fs::create_dir_all(&dest_dir).unwrap();

        let results = move_files(
            &[src.to_string_lossy().to_string()],
            &dest_dir.to_string_lossy(),
        )
        .unwrap();

        assert!(!src.exists());
        assert!(Path::new(&results[0]).exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_copy_files() {
        let base = temp_dir("copy");
        let src = base.join("file.txt");
        let dest_dir = base.join("target");
        File::create(&src).unwrap().write_all(b"data").unwrap();
        fs::create_dir_all(&dest_dir).unwrap();

        let results = copy_files(
            &[src.to_string_lossy().to_string()],
            &dest_dir.to_string_lossy(),
        )
        .unwrap();

        assert!(src.exists());
        assert!(Path::new(&results[0]).exists());
        assert_eq!(fs::read_to_string(&results[0]).unwrap(), "data");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_soft_delete() {
        let base = temp_dir("soft_delete");
        let file = base.join("doomed.txt");
        File::create(&file).unwrap().write_all(b"bye").unwrap();

        let results = soft_delete(&[file.to_string_lossy().to_string()]).unwrap();

        assert!(!file.exists());
        assert!(Path::new(&results[0].trash_path).exists());
        assert_eq!(fs::read_to_string(&results[0].trash_path).unwrap(), "bye");

        let trash_parent = Path::new(&results[0].trash_path).parent().unwrap();
        assert!(trash_parent.join("metadata.json").exists());

        let _ = fs::remove_dir_all(&base);
        let _ = fs::remove_dir_all(trash_parent);
    }

    #[test]
    fn test_restore_from_trash() {
        let base = temp_dir("restore");
        let file = base.join("restore_me.txt");
        File::create(&file).unwrap().write_all(b"back").unwrap();
        let original = file.to_string_lossy().to_string();

        let results = soft_delete(&[original.clone()]).unwrap();
        assert!(!file.exists());

        restore_from_trash(&results[0].trash_path, &original).unwrap();
        assert!(file.exists());
        assert_eq!(fs::read_to_string(&file).unwrap(), "back");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_rename_protected_path_rejected() {
        let result = rename("/bin/ls", "/tmp/ls_stolen");
        assert!(result.is_err());
    }
}
