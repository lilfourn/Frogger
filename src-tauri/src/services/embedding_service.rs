use std::sync::Mutex;

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use rusqlite::Connection;

use crate::data::repository;
use crate::error::AppError;

static MODEL: Mutex<Option<TextEmbedding>> = Mutex::new(None);

#[allow(dead_code)]
pub const EMBEDDING_DIMENSIONS: usize = 384;
const REEMBED_BATCH_SIZE: usize = 64;
const REEMBED_PROGRESS_BATCH_SIZE: usize = 10;

#[derive(Debug, Clone)]
pub struct EmbeddingDocument {
    pub file_path: String,
    pub file_name: String,
    pub extension: Option<String>,
    pub ocr_text: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ReembedReport {
    pub processed: usize,
    pub embedded: usize,
    pub skipped_missing: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReembedProgressState {
    pub status: String,
    pub processed: usize,
    pub total: usize,
    pub embedded: usize,
    pub skipped_missing: usize,
    pub failed: usize,
    pub message: String,
}

impl ReembedProgressState {
    pub fn idle() -> Self {
        Self {
            status: "idle".to_string(),
            processed: 0,
            total: 0,
            embedded: 0,
            skipped_missing: 0,
            failed: 0,
            message: "Idle".to_string(),
        }
    }
}

/// Initialize ONNX Runtime with hardware-accelerated execution providers.
/// Must be called before any TextEmbedding::try_new().
pub fn init_embedding_runtime() {
    use ort::execution_providers::ExecutionProviderDispatch;

    let mut eps: Vec<ExecutionProviderDispatch> = vec![];

    #[cfg(target_os = "macos")]
    eps.push(ort::ep::coreml::CoreML::default().build());

    eps.push(ort::ep::xnnpack::XNNPACK::default().build());

    if !eps.is_empty() {
        let _ = ort::init().with_execution_providers(eps).commit();
    }
}

fn build_embedding_text(
    file_name: &str,
    extension: Option<&str>,
    ocr_text: Option<&str>,
) -> String {
    let mut parts = vec![file_name.to_string()];
    if let Some(ext) = extension {
        if !ext.is_empty() {
            parts.push(ext.to_string());
        }
    }
    if let Some(text) = ocr_text {
        if !text.is_empty() {
            parts.push(text.to_string());
        }
    }
    parts.join(" ")
}

fn with_model<F, T>(f: F) -> Result<T, AppError>
where
    F: FnOnce(&mut TextEmbedding) -> Result<T, AppError>,
{
    let mut guard = MODEL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if guard.is_none() {
        let model = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::BGESmallENV15))
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

pub fn generate_query_embedding(_conn: &Connection, text: &str) -> Result<Vec<f32>, AppError> {
    generate_embedding(text)
}

pub fn embed_documents(conn: &Connection, docs: &[EmbeddingDocument]) -> Result<(), AppError> {
    if docs.is_empty() {
        return Ok(());
    }

    for doc in docs {
        let text = build_embedding_text(
            &doc.file_name,
            doc.extension.as_deref(),
            doc.ocr_text.as_deref(),
        );
        let embedding = generate_embedding(&text)?;
        repository::insert_vec(conn, &doc.file_path, &embedding)?;
    }

    Ok(())
}

pub fn reembed_indexed_files(conn: &Connection) -> Result<ReembedReport, AppError> {
    reembed_indexed_files_with_progress(conn, |_| {})
}

fn to_progress_state(
    status: &str,
    total: usize,
    report: &ReembedReport,
    message: String,
) -> ReembedProgressState {
    ReembedProgressState {
        status: status.to_string(),
        processed: report.processed,
        total,
        embedded: report.embedded,
        skipped_missing: report.skipped_missing,
        failed: report.failed,
        message,
    }
}

pub fn reembed_indexed_files_with_progress<F>(
    conn: &Connection,
    mut on_progress: F,
) -> Result<ReembedReport, AppError>
where
    F: FnMut(ReembedProgressState),
{
    let candidates = repository::list_embedding_candidates(conn)?;
    let total = candidates.len();
    let mut report = ReembedReport {
        processed: 0,
        embedded: 0,
        skipped_missing: 0,
        failed: 0,
    };

    on_progress(to_progress_state(
        "running",
        total,
        &report,
        format!("Preparing to rebuild embeddings for {total} indexed files..."),
    ));

    if total == 0 {
        on_progress(to_progress_state(
            "done",
            total,
            &report,
            "No indexed files to re-embed.".to_string(),
        ));
        return Ok(report);
    }

    let mut batch = Vec::<EmbeddingDocument>::new();

    for candidate in candidates {
        report.processed += 1;

        if std::fs::metadata(&candidate.file_path).is_err() {
            report.skipped_missing += 1;
            let _ = repository::delete_file_index(conn, &candidate.file_path);
            continue;
        }

        batch.push(EmbeddingDocument {
            file_path: candidate.file_path,
            file_name: candidate.file_name,
            extension: candidate.extension,
            ocr_text: candidate.ocr_text,
        });

        if batch.len() >= REEMBED_BATCH_SIZE {
            let batch_len = batch.len();
            let docs = std::mem::take(&mut batch);
            if embed_documents(conn, &docs).is_ok() {
                report.embedded += batch_len;
            } else {
                report.failed += batch_len;
            }
        }

        if report.processed.is_multiple_of(REEMBED_PROGRESS_BATCH_SIZE) || report.processed == total
        {
            on_progress(to_progress_state(
                "running",
                total,
                &report,
                format!(
                    "Rebuilding embeddings: {}/{} processed",
                    report.processed, total
                ),
            ));
        }
    }

    if !batch.is_empty() {
        let batch_len = batch.len();
        if embed_documents(conn, &batch).is_ok() {
            report.embedded += batch_len;
        } else {
            report.failed += batch_len;
        }
    }

    on_progress(to_progress_state(
        "done",
        total,
        &report,
        format!(
            "Rebuild complete: {} embedded, {} skipped missing, {} failed.",
            report.embedded, report.skipped_missing, report.failed
        ),
    ));

    Ok(report)
}

pub fn embed_file(
    conn: &Connection,
    file_path: &str,
    file_name: &str,
    extension: Option<&str>,
    ocr_text: Option<&str>,
) -> Result<(), AppError> {
    embed_documents(
        conn,
        &[EmbeddingDocument {
            file_path: file_path.to_string(),
            file_name: file_name.to_string(),
            extension: extension.map(ToString::to_string),
            ocr_text: ocr_text.map(ToString::to_string),
        }],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::migrations;

    fn test_conn() -> Connection {
        crate::data::register_sqlite_vec_extension();
        let conn = Connection::open_in_memory().unwrap();
        migrations::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn test_embedding_produces_384_dims() {
        let emb = generate_embedding("test document about files").unwrap();
        assert_eq!(emb.len(), EMBEDDING_DIMENSIONS);
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

    #[test]
    fn test_reembed_indexed_files_rebuilds_local_vectors() {
        let conn = test_conn();

        let dir = std::env::temp_dir().join("frogger_test_reembed_indexed_files");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("report.txt");
        std::fs::write(&file_path, "hello").unwrap();

        let entry = crate::models::file_entry::FileEntry {
            path: file_path.to_string_lossy().to_string(),
            name: "report.txt".to_string(),
            extension: Some("txt".to_string()),
            mime_type: Some("text/plain".to_string()),
            size_bytes: Some(5),
            created_at: None,
            modified_at: Some("2025-01-01T00:00:00Z".to_string()),
            is_directory: false,
            parent_path: Some(dir.to_string_lossy().to_string()),
        };
        repository::insert_file(&conn, &entry).unwrap();
        repository::insert_ocr_text(
            &conn,
            &entry.path,
            "setup instructions",
            "eng",
            None,
            "2025-01-01T00:00:00Z",
        )
        .unwrap();

        let report = reembed_indexed_files(&conn).unwrap();
        assert_eq!(report.processed, 1);
        assert_eq!(report.embedded, 1);
        assert_eq!(report.skipped_missing, 0);
        assert_eq!(report.failed, 0);

        let query = generate_embedding("setup instructions").unwrap();
        let results = repository::search_vec(&conn, &query, 5).unwrap();
        assert!(results.iter().any(|result| result.file_path == entry.path));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
