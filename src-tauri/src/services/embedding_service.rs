use std::sync::Mutex;

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use rusqlite::Connection;

use crate::data::repository;
use crate::error::AppError;

static MODEL: Mutex<Option<TextEmbedding>> = Mutex::new(None);

fn with_model<F, T>(f: F) -> Result<T, AppError>
where
    F: FnOnce(&mut TextEmbedding) -> Result<T, AppError>,
{
    let mut guard = MODEL.lock().unwrap();
    if guard.is_none() {
        let model = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::AllMiniLML6V2))
            .map_err(|e| AppError::Embedding(format!("model init failed: {e}")))?;
        *guard = Some(model);
    }
    f(guard.as_mut().unwrap())
}

pub fn generate_embedding(text: &str) -> Result<Vec<f32>, AppError> {
    with_model(|model| {
        let mut results = model
            .embed(vec![text], None)
            .map_err(|e| AppError::Embedding(e.to_string()))?;
        results
            .pop()
            .ok_or_else(|| AppError::Embedding("no embedding returned".to_string()))
    })
}

pub fn embed_file(
    conn: &Connection,
    file_path: &str,
    file_name: &str,
    extension: Option<&str>,
    ocr_text: Option<&str>,
) -> Result<(), AppError> {
    let mut parts = vec![file_name.to_string()];
    if let Some(ext) = extension {
        parts.push(ext.to_string());
    }
    if let Some(text) = ocr_text {
        if !text.is_empty() {
            parts.push(text.to_string());
        }
    }
    let combined = parts.join(" ");
    let embedding = generate_embedding(&combined)?;
    repository::insert_vec(conn, file_path, &embedding)?;
    Ok(())
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
    fn test_embedding_produces_384_dims() {
        let emb = generate_embedding("test document about files").unwrap();
        assert_eq!(emb.len(), 384);
    }

    #[test]
    fn test_embedding_consistency() {
        let emb1 = generate_embedding("consistent text input").unwrap();
        let emb2 = generate_embedding("consistent text input").unwrap();
        assert_eq!(emb1, emb2);
    }

    #[test]
    fn test_embed_file_stores_in_db() {
        let conn = test_conn();
        embed_file(&conn, "/test/doc.pdf", "doc.pdf", Some("pdf"), None).unwrap();

        let results =
            repository::search_vec(&conn, &generate_embedding("doc pdf").unwrap(), 5).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].file_path, "/test/doc.pdf");
    }
}
