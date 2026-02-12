use rusqlite::Connection;

use crate::data::repository;
use crate::error::AppError;
use crate::models::search::SearchResult;
use crate::services::embedding_service;

const MIN_SEMANTIC_QUERY_CHARS: usize = 2;

fn derive_file_name(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

fn resolve_file_metadata(conn: &Connection, path: &str) -> (String, bool) {
    if let Ok(Some(entry)) = repository::get_by_path(conn, path) {
        return (entry.name, entry.is_directory);
    }

    (derive_file_name(path), false)
}

pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<SearchResult>, AppError> {
    let trimmed = query.trim();
    if trimmed.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }

    let keyword_results = repository::search_file_paths_by_name_or_path(conn, trimmed, limit)?;

    if !keyword_results.is_empty() {
        let results = keyword_results
            .into_iter()
            .enumerate()
            .map(|(rank, result)| {
                let (file_name, is_directory) = resolve_file_metadata(conn, &result.file_path);
                SearchResult {
                    file_path: result.file_path,
                    file_name,
                    is_directory,
                    score: 1.0 / (rank as f64 + 1.0),
                    match_source: "fts".to_string(),
                    snippet: None,
                }
            })
            .collect();
        return Ok(results);
    }

    if trimmed.chars().count() < MIN_SEMANTIC_QUERY_CHARS {
        return Ok(Vec::new());
    }

    let vec_results = match embedding_service::generate_embedding(trimmed) {
        Ok(emb) => repository::search_vec(conn, &emb, limit).unwrap_or_default(),
        Err(_) => Vec::new(),
    };

    if vec_results.is_empty() {
        return Ok(Vec::new());
    }

    let results = vec_results
        .into_iter()
        .map(|result| {
            let (file_name, is_directory) = resolve_file_metadata(conn, &result.file_path);
            SearchResult {
                file_path: result.file_path,
                file_name,
                is_directory,
                score: 1.0 / (1.0 + result.distance.max(0.0)),
                match_source: "vec".to_string(),
                snippet: None,
            }
        })
        .collect();

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::migrations;
    use crate::models::file_entry::FileEntry;

    fn test_conn() -> Connection {
        crate::data::register_sqlite_vec_extension();
        let conn = Connection::open_in_memory().unwrap();
        migrations::run_migrations(&conn).unwrap();
        conn
    }

    fn insert_file(conn: &Connection, path: &str, name: &str, is_directory: bool, parent: &str) {
        let entry = FileEntry {
            path: path.to_string(),
            name: name.to_string(),
            extension: None,
            mime_type: None,
            size_bytes: Some(1),
            created_at: None,
            modified_at: Some("2025-01-01T00:00:00Z".to_string()),
            is_directory,
            parent_path: Some(parent.to_string()),
        };
        repository::insert_file(conn, &entry).unwrap();
    }

    #[test]
    fn test_keyword_first_returns_name_matches_before_semantic() {
        let conn = test_conn();

        insert_file(&conn, "/docs/invoices", "invoices", true, "/docs");
        insert_file(&conn, "/docs/invoices.txt", "invoices.txt", false, "/docs");

        let emb = embedding_service::generate_embedding("invoices").unwrap();
        repository::insert_vec(&conn, "/docs/notes.txt", &emb).unwrap();

        let results = search(&conn, "invoices", 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].file_path, "/docs/invoices");
        assert!(results[0].is_directory);
        assert_eq!(results[0].match_source, "fts");
        assert!(results.iter().all(|r| r.match_source == "fts"));
    }

    #[test]
    fn test_search_falls_back_to_semantic_after_keyword_miss() {
        let conn = test_conn();

        insert_file(&conn, "/docs/readme.md", "readme.md", false, "/docs");
        let emb = embedding_service::generate_embedding("setup instructions").unwrap();
        repository::insert_vec(&conn, "/docs/readme.md", &emb).unwrap();

        let results = search(&conn, "setup", 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].file_path, "/docs/readme.md");
        assert_eq!(results[0].match_source, "vec");
        assert!(!results[0].is_directory);
    }

    #[test]
    fn test_search_does_not_run_semantic_for_short_query_without_keyword_match() {
        let conn = test_conn();

        insert_file(&conn, "/docs/readme.md", "readme.md", false, "/docs");
        let emb = embedding_service::generate_embedding("readme setup instructions").unwrap();
        repository::insert_vec(&conn, "/docs/readme.md", &emb).unwrap();

        let results = search(&conn, "x", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_empty_query() {
        let conn = test_conn();
        let results = search(&conn, "", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_limit_applies_to_keyword_results() {
        let conn = test_conn();

        for i in 0..30 {
            let path = format!("/docs/common_file_{i}.txt");
            let name = format!("common_file_{i}.txt");
            insert_file(&conn, &path, &name, false, "/docs");
        }

        let results = search(&conn, "common", 5).unwrap();
        assert_eq!(results.len(), 5);
        assert!(results.iter().all(|r| r.match_source == "fts"));
    }
}
