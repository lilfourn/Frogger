use std::fs;
use std::path::Path;
use tauri::command;

use crate::error::AppError;
use crate::models::file_entry::FileEntry;

#[command]
pub fn list_directory(path: String) -> Result<Vec<FileEntry>, AppError> {
    let dir_path = Path::new(&path);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn test_list_directory_returns_entries() {
        let dir = std::env::temp_dir().join("frogger_test_list_dir");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        File::create(dir.join("file_a.txt")).unwrap();
        File::create(dir.join("file_b.md")).unwrap();
        fs::create_dir_all(dir.join("subdir")).unwrap();

        let result = list_directory(dir.to_string_lossy().to_string()).unwrap();

        assert_eq!(result.len(), 3);
        assert!(result[0].is_directory); // dirs first
        assert_eq!(result[0].name, "subdir");

        let names: Vec<&str> = result.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"file_a.txt"));
        assert!(names.contains(&"file_b.md"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_list_directory_invalid_path() {
        let result = list_directory("/nonexistent/path/1234567890".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_list_directory_populates_metadata() {
        let dir = std::env::temp_dir().join("frogger_test_list_meta");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("test.txt"), "hello world").unwrap();

        let result = list_directory(dir.to_string_lossy().to_string()).unwrap();
        let file = &result[0];

        assert_eq!(file.name, "test.txt");
        assert_eq!(file.extension.as_deref(), Some("txt"));
        assert_eq!(file.mime_type.as_deref(), Some("text/plain"));
        assert_eq!(file.size_bytes, Some(11)); // "hello world" = 11 bytes
        assert!(!file.is_directory);
        assert!(file.created_at.is_some());
        assert!(file.modified_at.is_some());
        assert_eq!(
            file.parent_path.as_deref(),
            Some(dir.to_string_lossy().as_ref())
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
