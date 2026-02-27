use crate::error::AppError;
use crate::shell::safety::{validate_not_protected, validate_path};
use serde::Serialize;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(unix)]
const EXDEV_CODE: i32 = 18;

#[cfg(windows)]
const EXDEV_CODE: i32 = 17;

#[cfg(not(any(unix, windows)))]
const EXDEV_CODE: i32 = -1;

fn is_cross_device_error(err: &std::io::Error) -> bool {
    err.raw_os_error().is_some_and(|code| code == EXDEV_CODE)
}

fn move_path_with_fallback(src: &Path, dest: &Path) -> Result<(), AppError> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }

    match fs::rename(src, dest) {
        Ok(()) => Ok(()),
        Err(e) if is_cross_device_error(&e) => {
            if src.is_dir() {
                copy_dir_recursive(src, dest)?;
                fs::remove_dir_all(src)?;
            } else {
                fs::copy(src, dest)?;
                fs::remove_file(src)?;
            }
            Ok(())
        }
        Err(e) if e.kind() == ErrorKind::NotFound && !src.exists() => Err(AppError::General(
            format!("source does not exist: {}", src.to_string_lossy()),
        )),
        Err(e) => Err(AppError::Io(e)),
    }
}

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
    validate_not_protected(destination)?;

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

    move_path_with_fallback(Path::new(source), Path::new(destination))?;
    Ok(())
}

pub fn move_files(sources: &[String], dest_dir: &str) -> Result<Vec<String>, AppError> {
    validate_path(dest_dir)?;
    validate_not_protected(dest_dir)?;
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
        if !src_path.exists() {
            return Err(AppError::General(format!("source does not exist: {src}")));
        }
        let file_name = src_path
            .file_name()
            .ok_or_else(|| AppError::General(format!("invalid source path: {src}")))?;
        let dest = Path::new(dest_dir).join(file_name);
        move_path_with_fallback(src_path, &dest)?;
        dest_paths.push(dest.to_string_lossy().to_string());
    }
    Ok(dest_paths)
}

pub fn copy_files(sources: &[String], dest_dir: &str) -> Result<Vec<String>, AppError> {
    validate_path(dest_dir)?;
    validate_not_protected(dest_dir)?;
    if !Path::new(dest_dir).is_dir() {
        return Err(AppError::General(format!(
            "destination is not a directory: {dest_dir}"
        )));
    }

    let mut dest_paths = Vec::new();
    for src in sources {
        validate_path(src)?;
        let src_path = Path::new(src);
        if !src_path.exists() {
            return Err(AppError::General(format!("source does not exist: {src}")));
        }
        let file_name = src_path
            .file_name()
            .ok_or_else(|| AppError::General(format!("invalid source path: {src}")))?;
        let dest = Path::new(dest_dir).join(file_name);

        if src_path.is_dir() {
            copy_dir_recursive(src_path, &dest)?;
        } else {
            fs::copy(src_path, &dest).map_err(|err| {
                if err.kind() == ErrorKind::NotFound && !src_path.exists() {
                    AppError::General(format!("source does not exist: {src}"))
                } else {
                    AppError::Io(err)
                }
            })?;
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

#[derive(Clone, Serialize)]
pub struct ProgressEvent {
    pub bytes_copied: u64,
    pub total_bytes: u64,
    pub current_file: String,
    pub percent: f64,
}

fn calculate_total_size(paths: &[String]) -> Result<u64, AppError> {
    let mut total = 0u64;
    for src in paths {
        let path = Path::new(src);
        if path.is_dir() {
            for entry in walkdir::WalkDir::new(path).into_iter().flatten() {
                if entry.file_type().is_file() {
                    total += entry.metadata().map(|m| m.len()).unwrap_or(0);
                }
            }
        } else {
            total += fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        }
    }
    Ok(total)
}

const COPY_BUF_SIZE: usize = 64 * 1024;

fn copy_file_with_progress(
    src: &Path,
    dest: &Path,
    bytes_copied: &mut u64,
    total_bytes: u64,
    cancel: &Arc<AtomicBool>,
    emit: &dyn Fn(ProgressEvent),
) -> Result<(), AppError> {
    let mut reader = fs::File::open(src)?;
    let mut writer = fs::File::create(dest)?;
    let mut buf = vec![0u8; COPY_BUF_SIZE];

    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = fs::remove_file(dest);
            return Err(AppError::General("operation cancelled".to_string()));
        }

        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n])?;
        *bytes_copied += n as u64;

        emit(ProgressEvent {
            bytes_copied: *bytes_copied,
            total_bytes,
            current_file: src
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            percent: if total_bytes > 0 {
                (*bytes_copied as f64 / total_bytes as f64) * 100.0
            } else {
                0.0
            },
        });
    }
    Ok(())
}

fn copy_dir_with_progress(
    src: &Path,
    dest: &Path,
    bytes_copied: &mut u64,
    total_bytes: u64,
    cancel: &Arc<AtomicBool>,
    emit: &dyn Fn(ProgressEvent),
) -> Result<(), AppError> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let dest_child = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_with_progress(
                &entry.path(),
                &dest_child,
                bytes_copied,
                total_bytes,
                cancel,
                emit,
            )?;
        } else {
            copy_file_with_progress(
                &entry.path(),
                &dest_child,
                bytes_copied,
                total_bytes,
                cancel,
                emit,
            )?;
        }
    }
    Ok(())
}

pub fn copy_files_with_progress(
    sources: &[String],
    dest_dir: &str,
    cancel: &Arc<AtomicBool>,
    emit: impl Fn(ProgressEvent),
) -> Result<Vec<String>, AppError> {
    validate_path(dest_dir)?;
    if !Path::new(dest_dir).is_dir() {
        return Err(AppError::General(format!(
            "destination is not a directory: {dest_dir}"
        )));
    }

    let total_bytes = calculate_total_size(sources)?;
    let mut bytes_copied = 0u64;
    let mut dest_paths = Vec::new();

    for src in sources {
        validate_path(src)?;
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::General("operation cancelled".to_string()));
        }

        let src_path = Path::new(src);
        let file_name = src_path
            .file_name()
            .ok_or_else(|| AppError::General(format!("invalid source path: {src}")))?;
        let dest = Path::new(dest_dir).join(file_name);

        if src_path.is_dir() {
            copy_dir_with_progress(
                src_path,
                &dest,
                &mut bytes_copied,
                total_bytes,
                cancel,
                &emit,
            )?;
        } else {
            copy_file_with_progress(
                src_path,
                &dest,
                &mut bytes_copied,
                total_bytes,
                cancel,
                &emit,
            )?;
        }
        dest_paths.push(dest.to_string_lossy().to_string());
    }
    Ok(dest_paths)
}

#[cfg(test)]
pub fn trash_dir() -> Result<std::path::PathBuf, AppError> {
    let test_trash = std::env::temp_dir().join("frogger-test-trash");
    fs::create_dir_all(&test_trash)?;
    Ok(test_trash)
}

#[cfg(not(test))]
pub fn trash_dir() -> Result<std::path::PathBuf, AppError> {
    if let Ok(custom) = std::env::var("FROGGER_TRASH_DIR") {
        let custom_path = Path::new(&custom).to_path_buf();
        fs::create_dir_all(&custom_path)?;
        return Ok(custom_path);
    }

    let home = dirs::home_dir()
        .ok_or_else(|| AppError::General("could not resolve home directory".to_string()))?;
    let trash = home.join(".frogger").join("trash");
    if fs::create_dir_all(&trash).is_ok() {
        let probe = trash.join(".frogger_write_probe");
        if fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&probe)
            .is_ok()
        {
            let _ = fs::remove_file(probe);
            return Ok(trash);
        }
    }

    let fallback = std::env::temp_dir().join("frogger-trash");
    fs::create_dir_all(&fallback)?;
    Ok(fallback)
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

        move_path_with_fallback(src_path, &dest)?;

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

    move_path_with_fallback(trash, original)?;

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
    fn test_move_files_missing_source_returns_clear_error() {
        let base = temp_dir("move_missing_source");
        let missing = base.join("missing.txt");
        let dest_dir = base.join("target");
        fs::create_dir_all(&dest_dir).unwrap();
        let missing_str = missing.to_string_lossy().to_string();

        let err = move_files(&[missing_str.clone()], &dest_dir.to_string_lossy()).unwrap_err();

        assert_eq!(
            err.to_string(),
            format!("source does not exist: {missing_str}")
        );
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
    fn test_copy_files_missing_source_returns_clear_error() {
        let base = temp_dir("copy_missing_source");
        let missing = base.join("missing.txt");
        let dest_dir = base.join("target");
        fs::create_dir_all(&dest_dir).unwrap();
        let missing_str = missing.to_string_lossy().to_string();

        let err = copy_files(&[missing_str.clone()], &dest_dir.to_string_lossy()).unwrap_err();

        assert_eq!(
            err.to_string(),
            format!("source does not exist: {missing_str}")
        );
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

    #[test]
    fn test_copy_with_progress_emits_events() {
        let base = temp_dir("copy_progress");
        let src = base.join("big.txt");
        let dest_dir = base.join("target");
        fs::create_dir_all(&dest_dir).unwrap();

        let data = vec![b'x'; 256 * 1024]; // 256KB
        File::create(&src).unwrap().write_all(&data).unwrap();

        let cancel = Arc::new(AtomicBool::new(false));
        let events = std::sync::Mutex::new(Vec::new());

        let results = copy_files_with_progress(
            &[src.to_string_lossy().to_string()],
            &dest_dir.to_string_lossy(),
            &cancel,
            |evt| events.lock().unwrap().push(evt),
        )
        .unwrap();

        assert_eq!(results.len(), 1);
        assert!(Path::new(&results[0]).exists());

        let captured = events.lock().unwrap();
        assert!(!captured.is_empty());
        let last = captured.last().unwrap();
        assert!((last.percent - 100.0).abs() < 0.01);
        assert_eq!(last.total_bytes, 256 * 1024);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_copy_with_progress_cancel() {
        let base = temp_dir("copy_cancel");
        let src = base.join("cancel_me.txt");
        let dest_dir = base.join("target");
        fs::create_dir_all(&dest_dir).unwrap();

        let data = vec![b'y'; 256 * 1024];
        File::create(&src).unwrap().write_all(&data).unwrap();

        let cancel = Arc::new(AtomicBool::new(true)); // pre-cancelled

        let result = copy_files_with_progress(
            &[src.to_string_lossy().to_string()],
            &dest_dir.to_string_lossy(),
            &cancel,
            |_| {},
        );

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("cancelled"));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_copy_dir_with_progress() {
        let base = temp_dir("copy_dir_progress");
        let src_dir = base.join("source_dir");
        let dest_dir = base.join("target");
        fs::create_dir_all(&src_dir).unwrap();
        fs::create_dir_all(&dest_dir).unwrap();
        File::create(src_dir.join("a.txt"))
            .unwrap()
            .write_all(b"aaa")
            .unwrap();
        File::create(src_dir.join("b.txt"))
            .unwrap()
            .write_all(b"bbb")
            .unwrap();

        let cancel = Arc::new(AtomicBool::new(false));
        let events = std::sync::Mutex::new(Vec::new());

        let results = copy_files_with_progress(
            &[src_dir.to_string_lossy().to_string()],
            &dest_dir.to_string_lossy(),
            &cancel,
            |evt| events.lock().unwrap().push(evt),
        )
        .unwrap();

        assert_eq!(results.len(), 1);
        assert!(Path::new(&results[0]).join("a.txt").exists());
        assert!(Path::new(&results[0]).join("b.txt").exists());

        let captured = events.lock().unwrap();
        assert!(captured.len() >= 2);

        let _ = fs::remove_dir_all(&base);
    }
}
