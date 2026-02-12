use std::path::Path;

use calamine::{open_workbook_auto, Reader};
use rusqlite::Connection;

use crate::error::AppError;
use crate::services::permission_service::{self, PermissionCapability};

const SPREADSHEET_EXTENSIONS: &[&str] = &["xls", "xlsx", "xlsm", "xlsb", "ods", "csv"];
const MAX_SHEETS: usize = 10;
const MAX_ROWS_PER_SHEET: usize = 200;
const MAX_COLS_PER_ROW: usize = 50;
const MAX_CHARS_PER_FILE: usize = 200_000;

fn normalized_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

fn push_bounded(out: &mut String, text: &str) -> bool {
    for ch in text.chars() {
        let next = ch.len_utf8();
        if out.len() + next > MAX_CHARS_PER_FILE {
            return false;
        }
        out.push(ch);
    }
    true
}

fn push_line_bounded(out: &mut String, line: &str) -> bool {
    push_bounded(out, line) && push_bounded(out, "\n")
}

fn finalize_output(mut out: String, truncated: bool) -> String {
    if truncated {
        let _ = push_line_bounded(&mut out, "[TRUNCATED]");
    }
    out
}

fn extract_csv_text(file_path: &str) -> Result<String, AppError> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_path(file_path)
        .map_err(|e| AppError::General(format!("csv open failed for '{file_path}': {e}")))?;

    let mut out = String::new();
    let mut truncated = false;
    if !push_line_bounded(&mut out, "sheet:csv") {
        return Ok(finalize_output(out, true));
    }

    if let Ok(headers) = reader.headers() {
        let row: Vec<&str> = headers.iter().take(MAX_COLS_PER_ROW).collect();
        if headers.len() > MAX_COLS_PER_ROW {
            truncated = true;
        }
        if !row.is_empty() {
            let line = format!("header: {}", row.join(" | "));
            if !push_line_bounded(&mut out, &line) {
                return Ok(finalize_output(out, true));
            }
        }
    }

    for (idx, record) in reader.records().enumerate() {
        if idx >= MAX_ROWS_PER_SHEET {
            truncated = true;
            break;
        }
        let record = record
            .map_err(|e| AppError::General(format!("csv read failed for '{file_path}': {e}")))?;
        if record.len() > MAX_COLS_PER_ROW {
            truncated = true;
        }
        let row: Vec<String> = record
            .iter()
            .take(MAX_COLS_PER_ROW)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect();
        if row.is_empty() {
            continue;
        }
        let line = format!("r{}: {}", idx + 1, row.join(" | "));
        if !push_line_bounded(&mut out, &line) {
            truncated = true;
            break;
        }
    }

    Ok(finalize_output(out, truncated))
}

fn extract_workbook_text(file_path: &str) -> Result<String, AppError> {
    let mut workbook = open_workbook_auto(file_path).map_err(|e| {
        AppError::General(format!("spreadsheet open failed for '{file_path}': {e}"))
    })?;
    let sheet_names = workbook.sheet_names().to_owned();

    let mut out = String::new();
    let mut truncated = false;

    for (sheet_idx, sheet_name) in sheet_names.iter().enumerate() {
        if sheet_idx >= MAX_SHEETS {
            truncated = true;
            break;
        }

        let Ok(range) = workbook.worksheet_range(sheet_name) else {
            continue;
        };
        let sheet_line = format!("sheet:{sheet_name}");
        if !push_line_bounded(&mut out, &sheet_line) {
            truncated = true;
            break;
        }

        for (row_idx, row) in range.rows().enumerate() {
            if row_idx >= MAX_ROWS_PER_SHEET {
                truncated = true;
                break;
            }

            let mut cells = Vec::new();
            for (col_idx, cell) in row.iter().enumerate() {
                if col_idx >= MAX_COLS_PER_ROW {
                    truncated = true;
                    break;
                }
                let value = cell.to_string();
                let value = value.trim();
                if !value.is_empty() {
                    cells.push(value.to_string());
                }
            }
            if cells.is_empty() {
                continue;
            }

            let line = format!("r{}: {}", row_idx + 1, cells.join(" | "));
            if !push_line_bounded(&mut out, &line) {
                truncated = true;
                break;
            }
        }

        if truncated {
            break;
        }
    }

    Ok(finalize_output(out, truncated))
}

pub fn is_spreadsheet_candidate(path: &Path) -> bool {
    normalized_extension(path)
        .as_deref()
        .is_some_and(|ext| SPREADSHEET_EXTENSIONS.contains(&ext))
}

pub fn extract_text(
    conn: &Connection,
    file_path: &str,
    allow_once: bool,
) -> Result<Option<String>, AppError> {
    if permission_service::enforce(
        conn,
        file_path,
        PermissionCapability::ContentScan,
        allow_once,
    )
    .is_err()
    {
        return Ok(None);
    }

    let path = Path::new(file_path);
    if !is_spreadsheet_candidate(path) {
        return Ok(None);
    }
    let Some(ext) = normalized_extension(path) else {
        return Ok(None);
    };

    let text = if ext == "csv" {
        extract_csv_text(file_path)?
    } else {
        extract_workbook_text(file_path)?
    };
    if text.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::migrations;
    use crate::data::repository;

    fn test_conn(default_mode: &str) -> Connection {
        crate::data::register_sqlite_vec_extension();
        let conn = Connection::open_in_memory().unwrap();
        migrations::run_migrations(&conn).unwrap();
        repository::set_setting(&conn, "permission_default_content_scan", default_mode).unwrap();
        repository::set_setting(&conn, "permission_default_modification", "allow").unwrap();
        repository::set_setting(&conn, "permission_default_ocr", "allow").unwrap();
        repository::set_setting(&conn, "permission_default_indexing", "allow").unwrap();
        conn
    }

    #[test]
    fn test_is_spreadsheet_candidate() {
        assert!(is_spreadsheet_candidate(Path::new("budget.xlsx")));
        assert!(is_spreadsheet_candidate(Path::new("snapshot.xlsm")));
        assert!(is_spreadsheet_candidate(Path::new("legacy.XLSB")));
        assert!(is_spreadsheet_candidate(Path::new("finance.ods")));
        assert!(is_spreadsheet_candidate(Path::new("data.csv")));
        assert!(!is_spreadsheet_candidate(Path::new("image.png")));
        assert!(!is_spreadsheet_candidate(Path::new("notes.txt")));
    }

    #[test]
    fn test_extract_csv_text() {
        let conn = test_conn("allow");
        let dir = std::env::temp_dir().join("frogger_test_extract_csv_text");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let csv_path = dir.join("budget.csv");
        std::fs::write(
            &csv_path,
            "name,amount,region\nalpha,100,north\nbeta,200,south\n",
        )
        .unwrap();

        let text = extract_text(&conn, csv_path.to_str().unwrap(), true)
            .unwrap()
            .unwrap();
        assert!(text.contains("sheet:csv"));
        assert!(text.contains("header: name | amount | region"));
        assert!(text.contains("r1: alpha | 100 | north"));
        assert!(text.contains("r2: beta | 200 | south"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_extract_text_respects_permissions() {
        let conn = test_conn("deny");
        let dir = std::env::temp_dir().join("frogger_test_extract_csv_deny");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let csv_path = dir.join("data.csv");
        std::fs::write(&csv_path, "k,v\nx,1\n").unwrap();

        let text = extract_text(&conn, csv_path.to_str().unwrap(), false).unwrap();
        assert!(text.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_extract_csv_text_truncates() {
        let conn = test_conn("allow");
        let dir = std::env::temp_dir().join("frogger_test_extract_csv_trunc");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let csv_path = dir.join("large.csv");

        let mut rows = String::from("col\n");
        for _ in 0..(MAX_ROWS_PER_SHEET + 50) {
            rows.push_str("value\n");
        }
        std::fs::write(&csv_path, rows).unwrap();

        let text = extract_text(&conn, csv_path.to_str().unwrap(), true)
            .unwrap()
            .unwrap();
        assert!(text.contains("[TRUNCATED]"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
