use std::path::Path;

use rusqlite::Connection;

use crate::data::repository;
use crate::error::AppError;
use crate::services::permission_service::{self, PermissionCapability};

const OCR_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "tiff", "bmp", "gif", "webp"];
const MIN_OCR_FILE_SIZE: u64 = 10_000;

pub fn is_ocr_candidate(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| OCR_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn should_process(conn: &Connection, file_path: &str, modified_at: &str) -> bool {
    match repository::get_ocr_text(conn, file_path) {
        Ok(Some(record)) => record.processed_at.as_str() < modified_at,
        _ => true,
    }
}

pub fn extract_text(image_path: &str) -> Result<(String, f64), AppError> {
    let mut lt = leptess::LepTess::new(None, "eng")
        .map_err(|e| AppError::Ocr(format!("init failed: {e}")))?;
    lt.set_image(image_path)
        .map_err(|e| AppError::Ocr(format!("set_image failed: {e}")))?;
    let text = lt
        .get_utf8_text()
        .map_err(|e| AppError::Ocr(format!("get_utf8_text failed: {e}")))?;
    let confidence = lt.mean_text_conf() as f64;
    Ok((text.trim().to_string(), confidence))
}

pub fn process_file(
    conn: &Connection,
    file_path: &str,
    file_name: &str,
    modified_at: &str,
) -> Result<Option<String>, AppError> {
    if permission_service::enforce(conn, file_path, PermissionCapability::Ocr, false).is_err() {
        return Ok(None);
    }

    if !is_ocr_candidate(Path::new(file_path)) {
        return Ok(None);
    }

    if let Ok(meta) = std::fs::metadata(file_path) {
        if meta.len() < MIN_OCR_FILE_SIZE {
            return Ok(None);
        }
    }

    if !should_process(conn, file_path, modified_at) {
        return Ok(repository::get_ocr_text(conn, file_path)?.map(|r| r.text_content));
    }

    let (text, confidence) = extract_text(file_path)?;
    if text.is_empty() {
        return Ok(None);
    }

    let now = chrono::Utc::now().to_rfc3339();
    repository::insert_ocr_text(conn, file_path, &text, "eng", Some(confidence), &now)?;
    repository::insert_fts(conn, file_path, file_name, &text)?;

    Ok(Some(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::migrations;
    use std::path::PathBuf;

    fn test_conn() -> Connection {
        crate::data::register_sqlite_vec_extension();
        let conn = Connection::open_in_memory().unwrap();
        migrations::run_migrations(&conn).unwrap();
        repository::set_setting(&conn, "permission_default_content_scan", "allow").unwrap();
        repository::set_setting(&conn, "permission_default_modification", "allow").unwrap();
        repository::set_setting(&conn, "permission_default_ocr", "allow").unwrap();
        repository::set_setting(&conn, "permission_default_indexing", "allow").unwrap();
        conn
    }

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("sample_ocr.png")
    }

    #[test]
    fn test_is_ocr_candidate() {
        assert!(is_ocr_candidate(Path::new("photo.png")));
        assert!(is_ocr_candidate(Path::new("scan.JPEG")));
        assert!(is_ocr_candidate(Path::new("image.webp")));
        assert!(!is_ocr_candidate(Path::new("readme.txt")));
        assert!(!is_ocr_candidate(Path::new("code.rs")));
        assert!(!is_ocr_candidate(Path::new("noext")));
    }

    #[test]
    fn test_extract_text_from_image() {
        let path = fixture_path();
        assert!(path.exists(), "fixture image missing: {}", path.display());

        let (text, confidence) = extract_text(path.to_str().unwrap()).unwrap();
        let lower = text.to_lowercase();
        assert!(
            lower.contains("hello") && lower.contains("frogger"),
            "expected 'Hello Frogger', got: {text}"
        );
        assert!(confidence > 0.0);
    }

    #[test]
    fn test_ocr_caching_skips_processed() {
        let conn = test_conn();
        let path = "/test/image.png";
        let now = chrono::Utc::now().to_rfc3339();

        repository::insert_ocr_text(&conn, path, "cached text", "eng", Some(90.0), &now).unwrap();

        // modified_at is older than processed_at → should NOT reprocess
        let old = "2020-01-01T00:00:00Z";
        assert!(!should_process(&conn, path, old));
    }

    #[test]
    fn test_ocr_reprocesses_when_newer() {
        let conn = test_conn();
        let path = "/test/image.png";
        let old = "2020-01-01T00:00:00Z";

        repository::insert_ocr_text(&conn, path, "old text", "eng", Some(80.0), &old).unwrap();

        let newer = "2025-06-01T00:00:00Z";
        assert!(should_process(&conn, path, newer));
    }

    #[test]
    fn test_ocr_skips_small_image() {
        let conn = test_conn();
        let dir = std::env::temp_dir().join("frogger_test_ocr_small");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let tiny = dir.join("tiny.png");
        std::fs::write(&tiny, &[0u8; 100]).unwrap();

        let result = process_file(
            &conn,
            tiny.to_str().unwrap(),
            "tiny.png",
            "2025-01-01T00:00:00Z",
        )
        .unwrap();
        assert!(result.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_ocr_skips_non_image() {
        let conn = test_conn();
        let result = process_file(
            &conn,
            "/test/readme.txt",
            "readme.txt",
            "2025-01-01T00:00:00Z",
        )
        .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_process_file_stores_in_db() {
        let conn = test_conn();
        let path = fixture_path();
        let path_str = path.to_str().unwrap();

        let result =
            process_file(&conn, path_str, "sample_ocr.png", "2025-01-01T00:00:00Z").unwrap();

        assert!(result.is_some());
        let text = result.unwrap().to_lowercase();
        assert!(text.contains("hello"));

        let ocr = repository::get_ocr_text(&conn, path_str).unwrap();
        assert!(ocr.is_some());

        let fts = repository::search_fts(&conn, "hello", 10).unwrap();
        assert!(!fts.is_empty());
    }
}
