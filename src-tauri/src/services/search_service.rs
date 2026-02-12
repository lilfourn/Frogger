use std::collections::HashMap;

use rusqlite::Connection;

use crate::data::repository;
use crate::error::AppError;
use crate::models::search::{FtsResult, SearchResult, VecResult};
use crate::services::embedding_service;

const RRF_K: f64 = 60.0;

pub fn rrf_fuse(
    fts_results: &[FtsResult],
    vec_results: &[VecResult],
    limit: usize,
) -> Vec<(String, f64)> {
    let mut scores: HashMap<String, f64> = HashMap::new();

    for (rank, result) in fts_results.iter().enumerate() {
        *scores.entry(result.file_path.clone()).or_default() += 1.0 / (RRF_K + rank as f64 + 1.0);
    }

    for (rank, result) in vec_results.iter().enumerate() {
        *scores.entry(result.file_path.clone()).or_default() += 1.0 / (RRF_K + rank as f64 + 1.0);
    }

    let mut ranked: Vec<(String, f64)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    ranked.truncate(limit);
    ranked
}

fn determine_source(path: &str, fts: &[FtsResult], vec: &[VecResult]) -> String {
    let in_fts = fts.iter().any(|r| r.file_path == path);
    let in_vec = vec.iter().any(|r| r.file_path == path);
    match (in_fts, in_vec) {
        (true, true) => "hybrid".to_string(),
        (true, false) => "fts".to_string(),
        (false, true) => "vec".to_string(),
        _ => "unknown".to_string(),
    }
}

pub fn hybrid_search(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, AppError> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let fts_limit = limit * 3;
    let vec_limit = limit * 3;

    let fts_results = repository::search_fts(conn, query, fts_limit).unwrap_or_default();

    let vec_results = match embedding_service::generate_embedding(query) {
        Ok(emb) => repository::search_vec(conn, &emb, vec_limit).unwrap_or_default(),
        Err(_) => Vec::new(),
    };

    if fts_results.is_empty() && vec_results.is_empty() {
        return Ok(Vec::new());
    }

    let fused = rrf_fuse(&fts_results, &vec_results, limit);

    let results = fused
        .into_iter()
        .map(|(path, score)| {
            let file_name = path.rsplit('/').next().unwrap_or(&path).to_string();
            let source = determine_source(&path, &fts_results, &vec_results);
            SearchResult {
                file_path: path,
                file_name,
                score,
                match_source: source,
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

    fn test_conn() -> Connection {
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
        let conn = Connection::open_in_memory().unwrap();
        migrations::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn test_rrf_fusion_ranking() {
        let fts = vec![
            FtsResult {
                file_path: "/a.txt".into(),
            },
            FtsResult {
                file_path: "/b.txt".into(),
            },
        ];
        let vec = vec![
            VecResult {
                file_path: "/b.txt".into(),
                distance: 0.1,
            },
            VecResult {
                file_path: "/c.txt".into(),
                distance: 0.3,
            },
        ];

        let fused = rrf_fuse(&fts, &vec, 10);

        // /b.txt appears in both → highest RRF score
        assert_eq!(fused[0].0, "/b.txt");
        assert!(fused[0].1 > fused[1].1);
        assert_eq!(fused.len(), 3);
    }

    #[test]
    fn test_hybrid_search_returns_results() {
        let conn = test_conn();

        repository::insert_fts(&conn, "/docs/readme.md", "readme.md", "setup instructions")
            .unwrap();
        let emb = embedding_service::generate_embedding("readme setup instructions").unwrap();
        repository::insert_vec(&conn, "/docs/readme.md", &emb).unwrap();

        let results = hybrid_search(&conn, "readme", 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].file_path, "/docs/readme.md");
        assert_eq!(results[0].match_source, "hybrid");
    }

    #[test]
    fn test_hybrid_search_fts_only() {
        let conn = test_conn();
        repository::insert_fts(&conn, "/notes.txt", "notes.txt", "grocery list").unwrap();

        let results = hybrid_search(&conn, "grocery", 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].match_source, "fts");
    }

    #[test]
    fn test_hybrid_search_empty_query() {
        let conn = test_conn();
        let results = hybrid_search(&conn, "", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_hybrid_search_limit() {
        let conn = test_conn();

        for i in 0..30 {
            let path = format!("/file_{i}.txt");
            let name = format!("file_{i}.txt");
            repository::insert_fts(&conn, &path, &name, "common search term").unwrap();
        }

        let results = hybrid_search(&conn, "common", 5).unwrap();
        assert_eq!(results.len(), 5);
    }
}
