use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::data::repository;
use crate::error::AppError;
use crate::services::claude_service::{self, ChatMessage};
use crate::services::file_service;
use crate::services::indexing_service;
use crate::services::organize_service;
use crate::services::permission_service::{self, PermissionCapability};
use crate::state::{AppState, OrganizeProgressPhase, OrganizeProgressState, PermissionPolicyCacheEntry};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const ORGANIZE_MAX_FILES: usize = organize_service::DEFAULT_MAX_FILES;
pub const ORGANIZE_CHUNK_SIZE: usize = organize_service::DEFAULT_CHUNK_SIZE;
const INDEXING_WEIGHT: f64 = 0.10;
const PLANNING_WEIGHT: f64 = 0.75;
const APPLYING_WEIGHT: f64 = 0.15;
const CHUNK_REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
const ORGANIZE_PARALLEL_CHUNK_REQUESTS: usize = 3;
const ORGANIZE_CHUNK_RETRY_ATTEMPTS: usize = 5;
const ORGANIZE_REFINEMENT_CHUNK_SIZE: usize = 40;
const PLANNING_FIRST_PASS_MAX_PERCENT: usize = 80;
const PLANNING_REFINEMENT_MAX_PERCENT: usize = 95;

// ---------------------------------------------------------------------------
// Pipeline context
// ---------------------------------------------------------------------------

pub struct PipelineCtx {
    pub app: AppHandle,
    pub session_id: String,
    pub root_path: String,
    pub api_key: String,
    pub cancel_flag: Arc<AtomicBool>,
}

// ---------------------------------------------------------------------------
// Stage result types
// ---------------------------------------------------------------------------

pub struct StageIndexResult {
    pub manifest: Vec<organize_service::IndexedManifestFile>,
    pub skipped_hidden: usize,
    pub skipped_organized: usize,
}

pub struct StageClassifyResult {
    pub merged: HashMap<String, Vec<String>>,
    pub source_by_path: HashMap<String, String>,
    pub subfolders_by_path: HashMap<String, Option<String>>,
    pub uncertain_files: Vec<organize_service::IndexedManifestFile>,
    pub tree_summary: serde_json::Value,
    pub deterministic: HashMap<String, organize_service::DeterministicClassification>,
}

pub struct StageLlmClassifyResult {
    pub merged: HashMap<String, Vec<String>>,
    pub source_by_path: HashMap<String, String>,
    pub subfolders_by_path: HashMap<String, Option<String>>,
    pub suggested_names_by_path: HashMap<String, Option<String>>,
    pub parse_failed_chunks: usize,
}

pub struct StageRefineResult {
    pub assignments: HashMap<String, String>,
    pub subfolders_by_path: HashMap<String, Option<String>>,
    pub suggested_names_by_path: HashMap<String, Option<String>>,
    pub source_by_path: HashMap<String, String>,
    pub parse_failed_chunks: usize,
    pub refinement_chunk_count: usize,
}

pub struct StagePackResult {
    pub placements: Vec<organize_service::OrganizePlacement>,
    pub packing_stats: organize_service::OrganizePackingStats,
}

// ---------------------------------------------------------------------------
// Progress helpers
// ---------------------------------------------------------------------------

fn phase_percent(processed: usize, total: usize) -> usize {
    if total == 0 {
        return 0;
    }
    ((processed as f64 / total as f64) * 100.0)
        .round()
        .clamp(0.0, 100.0) as usize
}

fn combined_percent_for_phase(phase: OrganizeProgressPhase, percent: usize) -> usize {
    match phase {
        OrganizeProgressPhase::Indexing => ((percent as f64) * INDEXING_WEIGHT).round() as usize,
        OrganizeProgressPhase::Planning => {
            (10.0 + (percent as f64) * PLANNING_WEIGHT).round() as usize
        }
        OrganizeProgressPhase::Applying => {
            (85.0 + (percent as f64) * APPLYING_WEIGHT).round() as usize
        }
        OrganizeProgressPhase::Done => 100,
        _ => percent,
    }
    .min(100)
}

pub struct EmitProgressParams<'a> {
    pub app: &'a AppHandle,
    pub state: &'a AppState,
    pub session_id: &'a str,
    pub root_path: &'a str,
    pub phase: OrganizeProgressPhase,
    pub processed: usize,
    pub total: usize,
    pub message: String,
}

pub fn emit_organize_progress(params: EmitProgressParams) {
    if let Some(payload) = record_organize_progress(
        params.state,
        params.session_id,
        params.root_path,
        params.phase,
        params.processed,
        params.total,
        params.message,
    ) {
        let _ = params.app.emit("organize-progress", payload);
    }
}

pub fn record_organize_progress(
    state: &AppState,
    session_id: &str,
    root_path: &str,
    phase: OrganizeProgressPhase,
    processed: usize,
    total: usize,
    message: String,
) -> Option<OrganizeProgressState> {
    let percent = phase_percent(processed, total);
    let previous = state.get_organize_status(session_id);
    if previous
        .as_ref()
        .is_some_and(|progress| progress.phase.is_terminal())
    {
        return None;
    }

    let combined_percent = combined_percent_for_phase(phase, percent);
    let combined_percent = previous.as_ref().map_or(combined_percent, |progress| {
        progress.combined_percent.max(combined_percent)
    });

    let payload = OrganizeProgressState {
        session_id: session_id.to_string(),
        root_path: root_path.to_string(),
        phase,
        processed,
        total,
        percent,
        combined_percent,
        message,
        sequence: state.next_organize_progress_sequence(),
    };

    state.set_organize_status(payload.clone());
    Some(payload)
}

pub fn is_organize_cancelled(cancel_flag: &AtomicBool) -> bool {
    cancel_flag.load(Ordering::Relaxed)
}

fn planning_percent_first_pass(processed: usize, total: usize) -> usize {
    if total == 0 {
        return 0;
    }
    ((processed as f64 / total as f64) * PLANNING_FIRST_PASS_MAX_PERCENT as f64)
        .round()
        .clamp(0.0, PLANNING_FIRST_PASS_MAX_PERCENT as f64) as usize
}

fn planning_percent_refinement(processed: usize, total: usize) -> usize {
    if total == 0 {
        return PLANNING_FIRST_PASS_MAX_PERCENT;
    }
    let span = PLANNING_REFINEMENT_MAX_PERCENT.saturating_sub(PLANNING_FIRST_PASS_MAX_PERCENT);
    let refined = ((processed as f64 / total as f64) * span as f64)
        .round()
        .clamp(0.0, span as f64) as usize;
    (PLANNING_FIRST_PASS_MAX_PERCENT + refined).min(PLANNING_REFINEMENT_MAX_PERCENT)
}

fn emit_planning_progress(
    app: &AppHandle,
    state: &AppState,
    session_id: &str,
    root_path: &str,
    planning_percent: usize,
    message: impl Into<String>,
) {
    emit_organize_progress(EmitProgressParams {
        app,
        state,
        session_id,
        root_path,
        phase: OrganizeProgressPhase::Planning,
        processed: planning_percent.min(100),
        total: 100,
        message: message.into(),
    });
}

// ---------------------------------------------------------------------------
// LLM communication helpers
// ---------------------------------------------------------------------------

async fn send_chunk_with_cancellation(
    api_key: String,
    messages: Vec<ChatMessage>,
    prompt: String,
    app: AppHandle,
    cancel_flag: Arc<AtomicBool>,
) -> Result<String, AppError> {
    let started = tokio::time::Instant::now();
    let mut task = tokio::spawn(async move {
        claude_service::send_message(
            &api_key,
            &messages,
            &prompt,
            &app,
            Some(0.0),
            Some(3072),
            false,
        )
        .await
    });

    loop {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(75)) => {
                if cancel_flag.load(Ordering::Relaxed) {
                    task.abort();
                    return Err(AppError::General("organization cancelled".to_string()));
                }
                if started.elapsed() >= CHUNK_REQUEST_TIMEOUT {
                    task.abort();
                    return Err(AppError::General("organization chunk timed out".to_string()));
                }
            }
            join = &mut task => {
                return match join {
                    Ok(inner) => inner,
                    Err(join_err) if join_err.is_cancelled() => {
                        Err(AppError::General("organization cancelled".to_string()))
                    }
                    Err(join_err) => Err(AppError::General(format!("organize chunk task failed: {join_err}"))),
                };
            }
        }
    }
}

fn is_retryable_chunk_error(message: &str) -> bool {
    let lower = message.to_lowercase();

    if lower.contains("organization cancelled") || lower.contains("cancelled") {
        return false;
    }

    if lower.contains("unauthorized")
        || lower.contains("bad request")
        || lower.contains("no api key")
        || lower.contains("invalid api key")
        || lower.contains("permission")
    {
        return false;
    }

    if let Some(status) = extract_http_status_code(&lower) {
        if (400..500).contains(&status) && !matches!(status, 408 | 409 | 429) {
            return false;
        }
    }

    true
}

fn extract_http_status_code(message: &str) -> Option<u16> {
    for token in message.split(|c: char| !c.is_ascii_digit()) {
        if token.len() != 3 {
            continue;
        }
        let Ok(status) = token.parse::<u16>() else {
            continue;
        };
        if (100..600).contains(&status) {
            return Some(status);
        }
    }
    None
}

fn retry_backoff_duration(attempt: usize) -> Duration {
    let exponent = (attempt.saturating_sub(1)).min(5) as u32;
    let base = 250u64
        .saturating_mul(2u64.saturating_pow(exponent))
        .min(5_000u64);
    let jitter_seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    let jitter = jitter_seed % 250;
    Duration::from_millis(base + jitter)
}

async fn sleep_with_cancellation(
    cancel_flag: &Arc<AtomicBool>,
    delay: Duration,
) -> Result<(), AppError> {
    let started = tokio::time::Instant::now();
    while started.elapsed() < delay {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err(AppError::General("organization cancelled".to_string()));
        }
        let remaining = delay.saturating_sub(started.elapsed());
        tokio::time::sleep(remaining.min(Duration::from_millis(100))).await;
    }
    Ok(())
}

async fn send_chunk_with_retry(
    api_key: String,
    messages: Vec<ChatMessage>,
    prompt: String,
    app: AppHandle,
    cancel_flag: Arc<AtomicBool>,
) -> Result<String, AppError> {
    let attempts = ORGANIZE_CHUNK_RETRY_ATTEMPTS.max(1);
    let mut last_error: Option<AppError> = None;

    for attempt in 1..=attempts {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err(AppError::General("organization cancelled".to_string()));
        }

        match send_chunk_with_cancellation(
            api_key.clone(),
            messages.clone(),
            prompt.clone(),
            app.clone(),
            cancel_flag.clone(),
        )
        .await
        {
            Ok(response) => return Ok(response),
            Err(err) => {
                let err_text = err.to_string();
                if err_text.contains("cancelled") {
                    return Err(AppError::General("organization cancelled".to_string()));
                }

                if attempt >= attempts || !is_retryable_chunk_error(&err_text) {
                    if attempt > 1 {
                        return Err(AppError::General(format!(
                            "organization chunk failed after {attempt} attempts: {err_text}"
                        )));
                    }
                    return Err(err);
                }

                last_error = Some(err);
                let delay = retry_backoff_duration(attempt);
                sleep_with_cancellation(&cancel_flag, delay).await?;
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        AppError::General("organization chunk failed with unknown error".to_string())
    }))
}

// ---------------------------------------------------------------------------
// Chunk batch classification
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ChunkClassificationJob {
    idx: usize,
    prompt: String,
    chunk_files: Vec<organize_service::IndexedManifestFile>,
    chunk_paths: HashSet<String>,
    deterministic: HashMap<String, organize_service::DeterministicClassification>,
}

#[derive(Debug, Clone)]
struct ChunkClassificationResult {
    idx: usize,
    parse_failed: bool,
    chunk_paths: HashSet<String>,
    map: HashMap<String, Vec<String>>,
    subfolders: HashMap<String, Option<String>>,
    suggested_names: HashMap<String, Option<String>>,
    fallback_paths: HashSet<String>,
}

async fn classify_chunk_batch<F>(
    jobs: Vec<ChunkClassificationJob>,
    api_key: &str,
    app: &AppHandle,
    cancel_flag: Arc<AtomicBool>,
    current_dir: &str,
    user_message: &str,
    mut on_complete: F,
) -> Result<Vec<ChunkClassificationResult>, AppError>
where
    F: FnMut(usize, usize, usize),
{
    if jobs.is_empty() {
        return Ok(Vec::new());
    }

    let total = jobs.len();
    let parallelism = ORGANIZE_PARALLEL_CHUNK_REQUESTS.max(1).min(total);
    let semaphore = Arc::new(Semaphore::new(parallelism));
    let mut join_set: JoinSet<Result<ChunkClassificationResult, AppError>> = JoinSet::new();

    for job in jobs {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| AppError::General("organization worker pool closed".to_string()))?;
        let api_key = api_key.to_string();
        let app = app.clone();
        let cancel_flag = cancel_flag.clone();
        let current_dir = current_dir.to_string();
        let user_message = user_message.to_string();

        join_set.spawn(async move {
            let _permit = permit;
            if cancel_flag.load(Ordering::Relaxed) {
                return Err(AppError::General("organization cancelled".to_string()));
            }

            let response = send_chunk_with_retry(
                api_key,
                vec![ChatMessage {
                    role: "user".to_string(),
                    content: user_message,
                }],
                job.prompt,
                app,
                cancel_flag.clone(),
            )
            .await
            .map_err(|e| {
                if e.to_string().contains("cancelled") {
                    return e;
                }
                sentry::configure_scope(|scope| {
                    scope.set_extra(
                        "organize_chunk",
                        serde_json::Value::String(format!("{}/{}", job.idx + 1, total)),
                    );
                    scope.set_extra(
                        "organize_dir",
                        serde_json::Value::String(current_dir.clone()),
                    );
                });
                e.capture()
            })?;

            if cancel_flag.load(Ordering::Relaxed) {
                return Err(AppError::General("organization cancelled".to_string()));
            }

            let mut parse_failed = false;
            let parsed = match organize_service::parse_chunk_plan(
                &response,
                &job.chunk_files,
                &job.deterministic,
            ) {
                Ok(parsed) => parsed,
                Err(parse_err) => {
                    parse_failed = true;
                    sentry::capture_message(
                        &format!(
                            "Organize chunk parse failed: {parse_err}. Falling back to deterministic classification."
                        ),
                        sentry::Level::Warning,
                    );
                    organize_service::fallback_chunk_plan(&job.chunk_files, &job.deterministic)
                }
            };

            Ok(ChunkClassificationResult {
                idx: job.idx,
                parse_failed,
                chunk_paths: job.chunk_paths,
                map: parsed.category_map,
                subfolders: parsed.subfolders,
                suggested_names: parsed.suggested_names,
                fallback_paths: parsed.fallback_paths,
            })
        });
    }

    let mut completed = 0usize;
    let mut ordered: Vec<Option<ChunkClassificationResult>> = (0..total).map(|_| None).collect();
    while let Some(joined) = join_set.join_next().await {
        if cancel_flag.load(Ordering::Relaxed) {
            join_set.abort_all();
            return Err(AppError::General("organization cancelled".to_string()));
        }

        let outcome = match joined {
            Ok(inner) => inner,
            Err(join_err) if join_err.is_cancelled() => {
                Err(AppError::General("organization cancelled".to_string()))
            }
            Err(join_err) => Err(AppError::General(format!(
                "organize chunk task failed: {join_err}"
            ))),
        };

        let result = match outcome {
            Ok(result) => result,
            Err(err) => {
                join_set.abort_all();
                return Err(err);
            }
        };

        if result.idx >= total {
            join_set.abort_all();
            return Err(AppError::General(format!(
                "chunk result index {} out of range",
                result.idx
            )));
        }
        if ordered[result.idx].is_some() {
            join_set.abort_all();
            return Err(AppError::General(format!(
                "duplicate chunk result for index {}",
                result.idx + 1
            )));
        }

        let idx = result.idx;
        completed += 1;
        on_complete(completed, total, idx);
        ordered[idx] = Some(result);
    }

    if completed != total {
        return Err(AppError::General(format!(
            "organization chunk processing incomplete: expected {total}, got {completed}"
        )));
    }

    let mut out = Vec::with_capacity(total);
    for (idx, maybe_result) in ordered.into_iter().enumerate() {
        let result = maybe_result.ok_or_else(|| {
            AppError::General(format!("missing chunk result for index {}", idx + 1))
        })?;
        out.push(result);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Action execution helpers
// ---------------------------------------------------------------------------

enum OrganizeExecutableAction {
    CreateDirectory {
        path: String,
    },
    MoveFiles {
        sources: Vec<String>,
        dest_dir: String,
    },
    CopyFiles {
        sources: Vec<String>,
        dest_dir: String,
    },
    RenameFile {
        source: String,
        destination: String,
    },
    DeleteFiles {
        paths: Vec<String>,
    },
}

fn get_string_arg(args: &serde_json::Value, key: &str) -> Result<String, AppError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(ToOwned::to_owned)
        .ok_or_else(|| AppError::General(format!("Invalid organize action args: missing '{key}'")))
}

fn get_string_vec_arg(args: &serde_json::Value, key: &str) -> Result<Vec<String>, AppError> {
    let Some(values) = args.get(key).and_then(|v| v.as_array()) else {
        return Err(AppError::General(format!(
            "Invalid organize action args: missing '{key}'"
        )));
    };
    values
        .iter()
        .map(|v| {
            v.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                AppError::General(format!(
                    "Invalid organize action args: '{key}' contains non-string value"
                ))
            })
        })
        .collect()
}

fn parse_organize_action(
    action: &organize_service::OrganizeAction,
) -> Result<OrganizeExecutableAction, AppError> {
    match action.tool.as_str() {
        "create_directory" => Ok(OrganizeExecutableAction::CreateDirectory {
            path: get_string_arg(&action.args, "path")?,
        }),
        "move_files" => Ok(OrganizeExecutableAction::MoveFiles {
            sources: get_string_vec_arg(&action.args, "sources")?,
            dest_dir: get_string_arg(&action.args, "dest_dir")?,
        }),
        "copy_files" => Ok(OrganizeExecutableAction::CopyFiles {
            sources: get_string_vec_arg(&action.args, "sources")?,
            dest_dir: get_string_arg(&action.args, "dest_dir")?,
        }),
        "rename_file" => Ok(OrganizeExecutableAction::RenameFile {
            source: get_string_arg(&action.args, "source")?,
            destination: get_string_arg(&action.args, "destination")?,
        }),
        "delete_files" => Ok(OrganizeExecutableAction::DeleteFiles {
            paths: get_string_vec_arg(&action.args, "paths")?,
        }),
        other => Err(AppError::General(format!(
            "Unsupported organize action tool: {other}"
        ))),
    }
}

fn enforce_organize_action(
    policy: &PermissionPolicyCacheEntry,
    action: &OrganizeExecutableAction,
    allow_once: bool,
) -> Result<(), AppError> {
    match action {
        OrganizeExecutableAction::CreateDirectory { path } => {
            permission_service::enforce_with_cached_policy(
                policy,
                path,
                PermissionCapability::Modification,
                allow_once,
            )
        }
        OrganizeExecutableAction::MoveFiles { sources, dest_dir }
        | OrganizeExecutableAction::CopyFiles { sources, dest_dir } => {
            permission_service::enforce_with_cached_policy(
                policy,
                dest_dir,
                PermissionCapability::Modification,
                allow_once,
            )?;
            for source in sources {
                permission_service::enforce_with_cached_policy(
                    policy,
                    source,
                    PermissionCapability::Modification,
                    allow_once,
                )?;
            }
            Ok(())
        }
        OrganizeExecutableAction::RenameFile {
            source,
            destination,
        } => {
            permission_service::enforce_with_cached_policy(
                policy,
                source,
                PermissionCapability::Modification,
                allow_once,
            )?;
            permission_service::enforce_with_cached_policy(
                policy,
                destination,
                PermissionCapability::Modification,
                allow_once,
            )
        }
        OrganizeExecutableAction::DeleteFiles { paths } => {
            for path in paths {
                permission_service::enforce_with_cached_policy(
                    policy,
                    path,
                    PermissionCapability::Modification,
                    allow_once,
                )?;
            }
            Ok(())
        }
    }
}

fn apply_organize_action(action: &OrganizeExecutableAction) -> Result<(), AppError> {
    match action {
        OrganizeExecutableAction::CreateDirectory { path } => file_service::create_dir(path),
        OrganizeExecutableAction::MoveFiles { sources, dest_dir } => {
            file_service::move_files(sources, dest_dir).map(|_| ())
        }
        OrganizeExecutableAction::CopyFiles { sources, dest_dir } => {
            file_service::copy_files(sources, dest_dir).map(|_| ())
        }
        OrganizeExecutableAction::RenameFile {
            source,
            destination,
        } => file_service::rename(source, destination),
        OrganizeExecutableAction::DeleteFiles { paths } => {
            file_service::soft_delete(paths).map(|_| ())
        }
    }
}

/// Filter out missing source files instead of aborting.
fn filter_missing_sources(
    action: OrganizeExecutableAction,
    index: usize,
    total: usize,
    warnings: &mut Vec<String>,
) -> Option<OrganizeExecutableAction> {
    match action {
        OrganizeExecutableAction::CreateDirectory { .. } => Some(action),
        OrganizeExecutableAction::MoveFiles {
            sources,
            dest_dir,
        } => {
            let (valid, missing): (Vec<_>, Vec<_>) =
                sources.into_iter().partition(|s| Path::new(s.as_str()).exists());
            for m in &missing {
                warnings.push(format!(
                    "Skipped missing source in move_files action {}/{}: {m}",
                    index + 1, total,
                ));
            }
            if valid.is_empty() { None } else {
                Some(OrganizeExecutableAction::MoveFiles { sources: valid, dest_dir })
            }
        }
        OrganizeExecutableAction::CopyFiles {
            sources,
            dest_dir,
        } => {
            let (valid, missing): (Vec<_>, Vec<_>) =
                sources.into_iter().partition(|s| Path::new(s.as_str()).exists());
            for m in &missing {
                warnings.push(format!(
                    "Skipped missing source in copy_files action {}/{}: {m}",
                    index + 1, total,
                ));
            }
            if valid.is_empty() { None } else {
                Some(OrganizeExecutableAction::CopyFiles { sources: valid, dest_dir })
            }
        }
        OrganizeExecutableAction::RenameFile { ref source, .. } => {
            if Path::new(source.as_str()).exists() {
                Some(action)
            } else {
                warnings.push(format!(
                    "Skipped missing source in rename_file action {}/{}: {source}",
                    index + 1, total,
                ));
                None
            }
        }
        OrganizeExecutableAction::DeleteFiles { paths } => {
            let (valid, missing): (Vec<_>, Vec<_>) =
                paths.into_iter().partition(|p| Path::new(p.as_str()).exists());
            for m in &missing {
                warnings.push(format!(
                    "Skipped missing path in delete_files action {}/{}: {m}",
                    index + 1, total,
                ));
            }
            if valid.is_empty() { None } else {
                Some(OrganizeExecutableAction::DeleteFiles { paths: valid })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// DB helpers
// ---------------------------------------------------------------------------

pub fn open_organize_scan_connection(state: &AppState) -> Result<rusqlite::Connection, AppError> {
    let conn = rusqlite::Connection::open(&state.db_path)
        .map_err(|e| AppError::General(format!("Failed to open DB for organize scan: {e}")))?;
    conn.busy_timeout(Duration::from_secs(5)).map_err(|e| {
        AppError::General(format!(
            "Failed to configure DB busy timeout for organize scan: {e}"
        ))
    })?;
    Ok(conn)
}

// ---------------------------------------------------------------------------
// Telemetry and error helpers
// ---------------------------------------------------------------------------

pub fn organize_error_kind(message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("database is locked") || lower.contains("database table is locked") {
        return "db_locked";
    }
    if lower.contains("cancelled") {
        return "cancelled";
    }
    if lower.contains("permission denied") || lower.contains("denied by user") {
        return "permission_denied";
    }
    if lower.contains("invalid organize plan payload") {
        return "invalid_plan_payload";
    }
    if lower.contains("missing source path") {
        return "missing_source_path";
    }
    "other"
}

fn summarize_organize_error(error_message: &str, max_chars: usize) -> String {
    let sanitized = error_message.replace(['\n', '\r'], " ");
    let mut iter = sanitized.chars();
    let mut out = String::new();
    for _ in 0..max_chars {
        let Some(ch) = iter.next() else {
            return out;
        };
        out.push(ch);
    }
    if iter.next().is_some() {
        out.push_str("...");
    }
    out
}

fn build_organize_audit_summary(
    operation: &str,
    outcome: &str,
    session_id: &str,
    action_count: Option<usize>,
    warning_count: Option<usize>,
    error_kind: Option<&str>,
    error_message: Option<&str>,
) -> String {
    let mut parts = vec![
        format!("operation={operation}"),
        format!("outcome={outcome}"),
        format!("session={session_id}"),
    ];

    if let Some(actions) = action_count {
        parts.push(format!("actions={actions}"));
    }
    if let Some(warnings) = warning_count {
        parts.push(format!("warnings={warnings}"));
    }
    if let Some(kind) = error_kind {
        parts.push(format!("error_kind={kind}"));
    }
    if let Some(message) = error_message {
        parts.push(format!("error={}", summarize_organize_error(message, 120)));
    }

    parts.join(" ")
}

pub struct OrganizeTelemetryEvent<'a> {
    pub endpoint: &'a str,
    pub operation: &'a str,
    pub outcome: &'a str,
    pub session_id: &'a str,
    pub action_count: Option<usize>,
    pub warning_count: Option<usize>,
    pub error_kind: Option<&'a str>,
    pub error_message: Option<&'a str>,
}

pub fn record_organize_telemetry(state: &AppState, event: OrganizeTelemetryEvent<'_>) {
    let summary = build_organize_audit_summary(
        event.operation,
        event.outcome,
        event.session_id,
        event.action_count,
        event.warning_count,
        event.error_kind,
        event.error_message,
    );

    if let Ok(conn) = state.db.lock() {
        let _ = repository::insert_audit_log(&conn, event.endpoint, Some(&summary), None, None);
    }

    if event.outcome == "error" {
        sentry::with_scope(
            |scope| {
                scope.set_tag("organize_operation", event.operation);
                scope.set_tag("organize_outcome", event.outcome);
                if let Some(kind) = event.error_kind {
                    scope.set_tag("organize_error_kind", kind);
                }
                scope.set_extra(
                    "organize_session_id",
                    serde_json::Value::String(event.session_id.to_string()),
                );
            },
            || {
                let level = if event.error_kind == Some("db_locked") {
                    sentry::Level::Warning
                } else {
                    sentry::Level::Error
                };
                sentry::capture_message(
                    &format!("organize/{} failed: {summary}", event.operation),
                    level,
                );
            },
        );
    }
}

// ---------------------------------------------------------------------------
// Stage: Index
// ---------------------------------------------------------------------------

pub fn stage_index(
    ctx: &PipelineCtx,
    state: &AppState,
    conn: &rusqlite::Connection,
    allow_once: bool,
) -> Result<StageIndexResult, AppError> {
    emit_organize_progress(EmitProgressParams {
        app: &ctx.app,
        state,
        session_id: &ctx.session_id,
        root_path: &ctx.root_path,
        phase: OrganizeProgressPhase::Indexing,
        processed: 0,
        total: 100,
        message: "Indexing directory tree...".into(),
    });

    {
        let db_conn = state
            .db
            .lock()
            .map_err(|e| AppError::General(e.to_string()))?;
        let policy = permission_service::load_policy_cache_entry(&db_conn, state)?;
        permission_service::enforce_with_cached_policy(
            &policy,
            &ctx.root_path,
            PermissionCapability::ContentScan,
            allow_once,
        )?;
        permission_service::enforce_with_cached_policy(
            &policy,
            &ctx.root_path,
            PermissionCapability::Indexing,
            allow_once,
        )?;
    }

    let cancelled = indexing_service::scan_directory_deep_with_progress_cancel(
        conn,
        &ctx.root_path,
        allow_once,
        ctx.cancel_flag.as_ref(),
        |processed, total| {
            emit_organize_progress(EmitProgressParams {
                app: &ctx.app,
                state,
                session_id: &ctx.session_id,
                root_path: &ctx.root_path,
                phase: OrganizeProgressPhase::Indexing,
                processed,
                total,
                message: format!("Indexing files {processed}/{total}"),
            });
        },
    );
    if cancelled || is_organize_cancelled(ctx.cancel_flag.as_ref()) {
        return Err(AppError::General("organization cancelled".to_string()));
    }

    let (manifest, skipped_hidden, skipped_organized) =
        organize_service::collect_indexed_manifest(
            conn,
            &ctx.root_path,
            false,
            ORGANIZE_MAX_FILES,
        )?;

    if manifest.is_empty() {
        return Err(AppError::General(
            "No indexed files found for this directory. Try indexing and retry.".to_string(),
        ));
    }

    Ok(StageIndexResult {
        manifest,
        skipped_hidden,
        skipped_organized,
    })
}

// ---------------------------------------------------------------------------
// Stage: Classify (deterministic split)
// ---------------------------------------------------------------------------

pub fn stage_classify(
    manifest: &[organize_service::IndexedManifestFile],
) -> StageClassifyResult {
    let tree_summary = organize_service::build_tree_summary(manifest);
    let deterministic = organize_service::classify_manifest_deterministic(manifest);
    let mut source_by_path: HashMap<String, String> = HashMap::new();
    let mut subfolders_by_path: HashMap<String, Option<String>> = HashMap::new();
    let mut merged: HashMap<String, Vec<String>> = HashMap::new();

    let mut uncertain_files = Vec::new();
    for file in manifest {
        let rel = file.relative_path.clone();
        let maybe_suggestion = deterministic.get(&rel);

        // Tier 1: high-confidence deterministic (uses default folder name)
        if let Some(suggestion) = maybe_suggestion {
            if suggestion.type_hint != organize_service::FileTypeHint::Unknown
                && suggestion.confidence >= organize_service::HIGH_CONFIDENCE_THRESHOLD
            {
                merged
                    .entry(suggestion.type_hint.default_folder_name().to_string())
                    .or_default()
                    .push(rel.clone());
                source_by_path.insert(rel.clone(), "deterministic".to_string());
                subfolders_by_path.insert(rel, None);
                continue;
            }
        }

        // Tier 2: fast classify for unambiguous types (smart folder name)
        if let Some(fast) = organize_service::classify_file_fast(file) {
            merged
                .entry(fast.folder)
                .or_default()
                .push(rel.clone());
            source_by_path.insert(rel.clone(), "fast".to_string());
            subfolders_by_path.insert(rel, None);
            continue;
        }

        // Tier 3: send to LLM
        uncertain_files.push(file.clone());
    }

    StageClassifyResult {
        merged,
        source_by_path,
        subfolders_by_path,
        uncertain_files,
        tree_summary,
        deterministic,
    }
}

// ---------------------------------------------------------------------------
// Stage: LLM Classification (first pass)
// ---------------------------------------------------------------------------

pub async fn stage_llm_classify(
    ctx: &PipelineCtx,
    state: &AppState,
    uncertain_files: &[organize_service::IndexedManifestFile],
    deterministic: &HashMap<String, organize_service::DeterministicClassification>,
    tree_summary: &serde_json::Value,
) -> Result<StageLlmClassifyResult, AppError> {
    let chunks = organize_service::chunk_manifest(uncertain_files, ORGANIZE_CHUNK_SIZE);
    emit_planning_progress(
        &ctx.app,
        state,
        &ctx.session_id,
        &ctx.root_path,
        2,
        format!(
            "Classifying {} uncertain files in {} chunks...",
            uncertain_files.len(),
            chunks.len()
        ),
    );

    let first_pass_jobs = chunks
        .iter()
        .enumerate()
        .map(|(idx, chunk)| {
            let chunk_deterministic = chunk
                .iter()
                .filter_map(|file| {
                    deterministic
                        .get(&file.relative_path)
                        .cloned()
                        .map(|decision| (file.relative_path.clone(), decision))
                })
                .collect::<HashMap<_, _>>();
            let prompt = organize_service::build_chunk_prompt(
                &ctx.root_path,
                idx + 1,
                chunks.len(),
                tree_summary,
                chunk,
                &chunk_deterministic,
            )?;
            let chunk_paths = chunk
                .iter()
                .map(|f| f.relative_path.clone())
                .collect::<HashSet<_>>();
            Ok(ChunkClassificationJob {
                idx,
                prompt,
                chunk_files: chunk.clone(),
                chunk_paths,
                deterministic: chunk_deterministic,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;

    let first_pass_results = classify_chunk_batch(
        first_pass_jobs,
        &ctx.api_key,
        &ctx.app,
        ctx.cancel_flag.clone(),
        &ctx.root_path,
        "Classify this file chunk into allowed categories.",
        |completed, total, latest_idx| {
            let planning_percent = planning_percent_first_pass(completed, total);
            emit_planning_progress(
                &ctx.app,
                state,
                &ctx.session_id,
                &ctx.root_path,
                planning_percent,
                format!(
                    "Analyzed chunk {completed}/{total} (latest: {}/{})",
                    latest_idx + 1,
                    total
                ),
            );
        },
    )
    .await?;

    if is_organize_cancelled(ctx.cancel_flag.as_ref()) {
        return Err(AppError::General("organization cancelled".to_string()));
    }

    let mut merged: HashMap<String, Vec<String>> = HashMap::new();
    let mut source_by_path: HashMap<String, String> = HashMap::new();
    let mut subfolders_by_path: HashMap<String, Option<String>> = HashMap::new();
    let mut suggested_names_by_path: HashMap<String, Option<String>> = HashMap::new();
    let mut parse_failed_chunks = 0usize;

    for result in &first_pass_results {
        if result.parse_failed {
            parse_failed_chunks += 1;
        }
        for (folder, files) in &result.map {
            merged
                .entry(folder.clone())
                .or_default()
                .extend(files.clone());
        }
        for (path, subfolder) in &result.subfolders {
            subfolders_by_path.insert(path.clone(), subfolder.clone());
        }
        for (path, name) in &result.suggested_names {
            suggested_names_by_path.insert(path.clone(), name.clone());
        }
        for rel in &result.chunk_paths {
            let source = if result.fallback_paths.contains(rel) {
                "fallback"
            } else {
                "llm"
            };
            source_by_path.insert(rel.clone(), source.to_string());
        }
    }

    emit_planning_progress(
        &ctx.app,
        state,
        &ctx.session_id,
        &ctx.root_path,
        PLANNING_FIRST_PASS_MAX_PERCENT,
        "Classification complete. Checking unclassified ratio...",
    );

    Ok(StageLlmClassifyResult {
        merged,
        source_by_path,
        subfolders_by_path,
        suggested_names_by_path,
        parse_failed_chunks,
    })
}

// ---------------------------------------------------------------------------
// Stage: Refine "other" assignments
// ---------------------------------------------------------------------------

pub async fn stage_refine_other(
    ctx: &PipelineCtx,
    state: &AppState,
    manifest: &[organize_service::IndexedManifestFile],
    assignments: &HashMap<String, String>,
    deterministic: &HashMap<String, organize_service::DeterministicClassification>,
    tree_summary: &serde_json::Value,
    prev_subfolders: &HashMap<String, Option<String>>,
    prev_suggested: &HashMap<String, Option<String>>,
    prev_source: &HashMap<String, String>,
    prev_parse_failed: usize,
) -> Result<StageRefineResult, AppError> {
    let mut out_assignments = assignments.clone();
    let mut out_subfolders = prev_subfolders.clone();
    let mut out_suggested = prev_suggested.clone();
    let mut out_source = prev_source.clone();
    let mut parse_failed_chunks = prev_parse_failed;
    let mut refinement_chunk_count = 0usize;

    let other_count = out_assignments
        .values()
        .filter(|folder| *folder == "other" || *folder == "uncategorized")
        .count();
    let other_ratio = if manifest.is_empty() {
        0.0
    } else {
        other_count as f64 / manifest.len() as f64
    };

    if other_ratio > 0.10 {
        let other_files = out_assignments
            .iter()
            .filter(|(_, folder)| *folder == "other" || *folder == "uncategorized")
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>();

        if !other_files.is_empty() {
            let manifest_index = manifest
                .iter()
                .map(|f| (f.relative_path.clone(), f.clone()))
                .collect::<HashMap<_, _>>();
            let mut refinement_files = other_files
                .iter()
                .filter_map(|rel| manifest_index.get(rel).cloned())
                .collect::<Vec<_>>();
            refinement_files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
            refinement_files.dedup_by(|a, b| a.relative_path == b.relative_path);

            let refinement_chunks = organize_service::chunk_manifest(
                &refinement_files,
                ORGANIZE_REFINEMENT_CHUNK_SIZE,
            );
            refinement_chunk_count = refinement_chunks.len();

            if refinement_chunk_count > 0 {
                emit_planning_progress(
                    &ctx.app,
                    state,
                    &ctx.session_id,
                    &ctx.root_path,
                    PLANNING_FIRST_PASS_MAX_PERCENT,
                    format!(
                        "Running de-other refinement across {} chunk(s)...",
                        refinement_chunk_count
                    ),
                );

                let refinement_jobs = refinement_chunks
                    .iter()
                    .enumerate()
                    .map(|(idx, chunk)| {
                        let chunk_deterministic = chunk
                            .iter()
                            .filter_map(|file| {
                                deterministic
                                    .get(&file.relative_path)
                                    .cloned()
                                    .map(|decision| (file.relative_path.clone(), decision))
                            })
                            .collect::<HashMap<_, _>>();
                        let prompt = organize_service::build_refinement_chunk_prompt(
                            &ctx.root_path,
                            idx + 1,
                            refinement_chunks.len(),
                            tree_summary,
                            chunk,
                            &out_assignments,
                            &chunk_deterministic,
                        )?;
                        let chunk_paths = chunk
                            .iter()
                            .map(|f| f.relative_path.clone())
                            .collect::<HashSet<_>>();
                        Ok(ChunkClassificationJob {
                            idx,
                            prompt,
                            chunk_files: chunk.clone(),
                            chunk_paths,
                            deterministic: chunk_deterministic,
                        })
                    })
                    .collect::<Result<Vec<_>, AppError>>()?;

                let refinement_results = classify_chunk_batch(
                    refinement_jobs,
                    &ctx.api_key,
                    &ctx.app,
                    ctx.cancel_flag.clone(),
                    &ctx.root_path,
                    "Refine unclassified files and move out of other when evidence is clear.",
                    |completed, total, _| {
                        let planning_percent = planning_percent_refinement(completed, total);
                        emit_planning_progress(
                            &ctx.app,
                            state,
                            &ctx.session_id,
                            &ctx.root_path,
                            planning_percent,
                            format!("Refined unclassified chunk {completed}/{total}"),
                        );
                    },
                )
                .await?;

                for result in refinement_results {
                    if result.parse_failed {
                        parse_failed_chunks += 1;
                    }

                    let result_assignments =
                        organize_service::category_assignments(&result.map);
                    for (rel, folder) in result_assignments {
                        if folder == "other" || folder == "uncategorized" {
                            out_assignments.entry(rel.clone()).or_insert(folder);
                        } else {
                            out_assignments.insert(rel.clone(), folder);
                        }
                    }

                    for (path, subfolder) in result.subfolders {
                        out_subfolders.insert(path, subfolder);
                    }
                    for (path, name) in result.suggested_names {
                        out_suggested.insert(path, name);
                    }
                    for rel in &result.chunk_paths {
                        if result.fallback_paths.contains(rel) {
                            out_source
                                .entry(rel.clone())
                                .or_insert_with(|| "fallback".to_string());
                        } else {
                            out_source.insert(rel.clone(), "llm".to_string());
                        }
                    }
                }
            }
        }
    }

    Ok(StageRefineResult {
        assignments: out_assignments,
        subfolders_by_path: out_subfolders,
        suggested_names_by_path: out_suggested,
        source_by_path: out_source,
        parse_failed_chunks,
        refinement_chunk_count,
    })
}

// ---------------------------------------------------------------------------
// Stage: Packing
// ---------------------------------------------------------------------------

pub async fn stage_pack(
    ctx: &PipelineCtx,
    state: &AppState,
    placements: Vec<organize_service::OrganizePlacement>,
    manifest: &[organize_service::IndexedManifestFile],
) -> Result<StagePackResult, AppError> {
    if is_organize_cancelled(ctx.cancel_flag.as_ref()) {
        return Err(AppError::General("organization cancelled".to_string()));
    }

    emit_planning_progress(
        &ctx.app,
        state,
        &ctx.session_id,
        &ctx.root_path,
        PLANNING_REFINEMENT_MAX_PERCENT,
        "Packing directories",
    );

    let (placements, packing_stats) =
        organize_service::apply_capacity_packing(placements, manifest);

    Ok(StagePackResult {
        placements,
        packing_stats,
    })
}

// ---------------------------------------------------------------------------
// Stage: Build plan document
// ---------------------------------------------------------------------------

pub fn stage_build_plan(
    manifest: &[organize_service::IndexedManifestFile],
    assignments: &HashMap<String, String>,
    _deterministic: &HashMap<String, organize_service::DeterministicClassification>,
    source_by_path: &HashMap<String, String>,
    _subfolders_by_path: &HashMap<String, Option<String>>,
    _suggested_names_by_path: &HashMap<String, Option<String>>,
    placements_packed: Vec<organize_service::OrganizePlacement>,
    packing_stats: organize_service::OrganizePackingStats,
    skipped_hidden: usize,
    skipped_organized: usize,
    chunk_count: usize,
    refinement_chunk_count: usize,
    parse_failed_chunks: usize,
) -> organize_service::OrganizePlanDocument {
    let merged = organize_service::assignments_to_category_map(assignments);

    let other_count = assignments
        .values()
        .filter(|folder| *folder == "other" || *folder == "uncategorized")
        .count();
    let other_ratio = if manifest.is_empty() {
        0.0
    } else {
        other_count as f64 / manifest.len() as f64
    };
    let deterministic_assigned = source_by_path
        .values()
        .filter(|source| source.as_str() == "deterministic")
        .count();
    let fast_classified = source_by_path
        .values()
        .filter(|source| source.as_str() == "fast")
        .count();
    let llm_assigned = source_by_path
        .values()
        .filter(|source| source.as_str() == "llm")
        .count();
    let fallback_assigned = source_by_path
        .values()
        .filter(|source| source.as_str() == "fallback")
        .count();

    let stats = organize_service::OrganizePlanStats {
        total_files: manifest.len(),
        indexed_files: manifest.len(),
        skipped_hidden,
        skipped_already_organized: skipped_organized,
        chunks: chunk_count + refinement_chunk_count,
        other_count,
        other_ratio,
        deterministic_assigned,
        fast_classified,
        llm_assigned,
        fallback_assigned,
        parse_failed_chunks,
        packed_directories: packing_stats.packed_directories,
        max_children_observed: packing_stats.max_children_observed,
        avg_children_per_generated_dir: packing_stats.avg_children_per_generated_dir,
        capacity_overflow_dirs: packing_stats.capacity_overflow_dirs,
        packing_llm_calls: packing_stats.packing_llm_calls,
        folders_over_target: packing_stats.folders_over_target,
        folders_over_hard_max: packing_stats.folders_over_hard_max,
        avg_depth_generated: packing_stats.avg_depth_generated,
        fallback_label_rate: packing_stats.fallback_label_rate,
    };

    organize_service::build_plan_document(merged, placements_packed, stats)
}

// ---------------------------------------------------------------------------
// Plan pipeline orchestrator
// ---------------------------------------------------------------------------

pub async fn run_plan_pipeline(
    ctx: &PipelineCtx,
    state: &AppState,
) -> Result<String, AppError> {
    // Scope the Connection so it is dropped before any .await points.
    // rusqlite::Connection is not Sync, so it cannot be held across awaits.
    let (index_result, classify_result, chunk_count) = {
        let conn = open_organize_scan_connection(state)?;
        let index_result = stage_index(ctx, state, &conn, true)?;
        let classify_result = stage_classify(&index_result.manifest);
        let chunk_count = organize_service::chunk_manifest(
            &classify_result.uncertain_files,
            ORGANIZE_CHUNK_SIZE,
        )
        .len();
        (index_result, classify_result, chunk_count)
    };

    if is_organize_cancelled(ctx.cancel_flag.as_ref()) {
        return Err(AppError::General("organization cancelled".to_string()));
    }

    // Stage 3: LLM classification
    let llm_result = stage_llm_classify(
        ctx,
        state,
        &classify_result.uncertain_files,
        &classify_result.deterministic,
        &classify_result.tree_summary,
    )
    .await?;

    // Merge deterministic + LLM results
    let mut merged = classify_result.merged;
    for (folder, files) in &llm_result.merged {
        merged.entry(folder.clone()).or_default().extend(files.clone());
    }
    let mut source_by_path = classify_result.source_by_path;
    source_by_path.extend(llm_result.source_by_path.clone());
    let mut subfolders_by_path = classify_result.subfolders_by_path;
    subfolders_by_path.extend(llm_result.subfolders_by_path.clone());

    let assignments = organize_service::category_assignments(&merged);

    // Stage 4: Refine "other"
    let refine_result = stage_refine_other(
        ctx,
        state,
        &index_result.manifest,
        &assignments,
        &classify_result.deterministic,
        &classify_result.tree_summary,
        &subfolders_by_path,
        &llm_result.suggested_names_by_path,
        &source_by_path,
        llm_result.parse_failed_chunks,
    )
    .await?;

    let final_merged = organize_service::assignments_to_category_map(&refine_result.assignments);
    let _ = &final_merged; // used below in placements

    emit_planning_progress(
        &ctx.app,
        state,
        &ctx.session_id,
        &ctx.root_path,
        PLANNING_REFINEMENT_MAX_PERCENT,
        "Finalizing organization plan...",
    );

    // Ensure all files have a source
    let mut final_source = refine_result.source_by_path;
    for file in &index_result.manifest {
        final_source
            .entry(file.relative_path.clone())
            .or_insert_with(|| "fallback".to_string());
    }

    // Build placements
    let placements = index_result
        .manifest
        .iter()
        .map(|file| {
            let rel = file.relative_path.clone();
            let folder = refine_result
                .assignments
                .get(&rel)
                .cloned()
                .unwrap_or_else(|| "uncategorized".to_string());
            let deterministic_hint = classify_result.deterministic.get(&rel);
            let subfolder = refine_result.subfolders_by_path.get(&rel).cloned().flatten();
            let source = final_source
                .get(&rel)
                .cloned()
                .unwrap_or_else(|| "fallback".to_string());
            let confidence = match source.as_str() {
                "deterministic" => deterministic_hint.map_or(0.8, |hint| hint.confidence),
                "llm" => 0.8,
                _ => deterministic_hint.map_or(0.45, |hint| hint.confidence.min(0.70)),
            };
            let suggested_name = refine_result
                .suggested_names_by_path
                .get(&rel)
                .cloned()
                .flatten();
            organize_service::OrganizePlacement {
                path: rel,
                folder,
                subfolder,
                packing_path: None,
                suggested_name,
                confidence,
                source,
            }
        })
        .collect::<Vec<_>>();

    // Stage 5: Packing
    let pack_result = stage_pack(ctx, state, placements, &index_result.manifest).await?;

    // Stage 6: Build plan document
    let plan = stage_build_plan(
        &index_result.manifest,
        &refine_result.assignments,
        &classify_result.deterministic,
        &final_source,
        &refine_result.subfolders_by_path,
        &refine_result.suggested_names_by_path,
        pack_result.placements,
        pack_result.packing_stats,
        index_result.skipped_hidden,
        index_result.skipped_organized,
        chunk_count,
        refine_result.refinement_chunk_count,
        refine_result.parse_failed_chunks,
    );

    emit_organize_progress(EmitProgressParams {
        app: &ctx.app,
        state,
        session_id: &ctx.session_id,
        root_path: &ctx.root_path,
        phase: OrganizeProgressPhase::Planning,
        processed: 100,
        total: 100,
        message: if plan.stats.other_ratio > 0.10 {
            format!(
                "Plan ready for review. Warning: {:.1}% remains uncategorized.",
                plan.stats.other_ratio * 100.0
            )
        } else {
            "Plan ready for review.".to_string()
        },
    });

    organize_service::plan_to_json_block(&plan)
}

// ---------------------------------------------------------------------------
// Execute pipeline orchestrator
// ---------------------------------------------------------------------------

pub fn run_execute_pipeline(
    ctx: &PipelineCtx,
    state: &AppState,
    plan_json: &str,
    allow_once: bool,
) -> Result<(String, usize, usize), AppError> {
    let conn = open_organize_scan_connection(state)?;

    {
        let db_conn = state
            .db
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let policy = permission_service::load_policy_cache_entry(&db_conn, state)?;
        permission_service::enforce_with_cached_policy(
            &policy,
            &ctx.root_path,
            PermissionCapability::ContentScan,
            allow_once,
        )?;
    }

    let (manifest, _, _) = organize_service::collect_indexed_manifest(
        &conn,
        &ctx.root_path,
        true,
        ORGANIZE_MAX_FILES,
    )?;
    let existing_paths = manifest
        .into_iter()
        .map(|file| file.absolute_path)
        .collect::<HashSet<String>>();

    if is_organize_cancelled(ctx.cancel_flag.as_ref()) {
        return Err(AppError::General("organization cancelled".to_string()));
    }

    let payload = organize_service::extract_json_payload(plan_json)
        .ok_or_else(|| AppError::General("Invalid organize plan payload".to_string()))?;
    let parsed_plan: organize_service::OrganizePlan = serde_json::from_str(&payload)?;

    let batch =
        organize_service::build_action_batch(&ctx.root_path, &parsed_plan, &existing_paths);
    let action_count = batch.actions.len();
    let warning_count = batch.warnings.len();
    if is_organize_cancelled(ctx.cancel_flag.as_ref()) {
        return Err(AppError::General("organization cancelled".to_string()));
    }
    let response = organize_service::actions_to_blocks(&batch)?;
    Ok((response, action_count, warning_count))
}

// ---------------------------------------------------------------------------
// Apply pipeline orchestrator
// ---------------------------------------------------------------------------

pub fn run_apply_pipeline(
    ctx: &PipelineCtx,
    state: &AppState,
    plan_json: &str,
    allow_once: bool,
) -> Result<(String, usize, usize), AppError> {
    let conn = open_organize_scan_connection(state)?;

    let policy = {
        let db_conn = state
            .db
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let policy = permission_service::load_policy_cache_entry(&db_conn, state)?;
        permission_service::enforce_with_cached_policy(
            &policy,
            &ctx.root_path,
            PermissionCapability::Modification,
            allow_once,
        )?;
        policy
    };

    let (manifest, _, _) = organize_service::collect_indexed_manifest(
        &conn,
        &ctx.root_path,
        true,
        ORGANIZE_MAX_FILES,
    )?;
    let existing_paths = manifest
        .into_iter()
        .map(|file| file.absolute_path)
        .collect::<HashSet<String>>();

    let payload = organize_service::extract_json_payload(plan_json)
        .ok_or_else(|| AppError::General("Invalid organize plan payload".to_string()))?;
    let parsed_plan: organize_service::OrganizePlan = serde_json::from_str(&payload)?;
    let batch =
        organize_service::build_action_batch(&ctx.root_path, &parsed_plan, &existing_paths);
    let mut warnings = batch.warnings;
    let batch = batch.actions;

    if batch.is_empty() {
        let warning_text = if warnings.is_empty() {
            "No safe actions were generated for apply.".to_string()
        } else {
            warnings.join("\n")
        };
        return Err(AppError::General(warning_text));
    }

    let total = batch.len();
    let mut skipped = 0usize;
    let mut parsed_actions = Vec::with_capacity(total);
    for (idx, raw_action) in batch.iter().enumerate() {
        if is_organize_cancelled(ctx.cancel_flag.as_ref()) {
            return Err(AppError::General("organization cancelled".to_string()));
        }

        let parsed_action = parse_organize_action(raw_action)?;
        enforce_organize_action(&policy, &parsed_action, allow_once)?;
        match filter_missing_sources(parsed_action, idx, total, &mut warnings) {
            Some(filtered) => parsed_actions.push(filtered),
            None => { skipped += 1; continue; }
        }
    }

    let apply_total = parsed_actions.len();
    emit_organize_progress(EmitProgressParams {
        app: &ctx.app,
        state,
        session_id: &ctx.session_id,
        root_path: &ctx.root_path,
        phase: OrganizeProgressPhase::Applying,
        processed: 0,
        total: apply_total,
        message: format!("Applying 0/{apply_total} actions..."),
    });

    for (idx, parsed_action) in parsed_actions.iter().enumerate() {
        if is_organize_cancelled(ctx.cancel_flag.as_ref()) {
            return Err(AppError::General("organization cancelled".to_string()));
        }

        apply_organize_action(parsed_action)?;

        let processed = idx + 1;
        emit_organize_progress(EmitProgressParams {
            app: &ctx.app,
            state,
            session_id: &ctx.session_id,
            root_path: &ctx.root_path,
            phase: OrganizeProgressPhase::Applying,
            processed,
            total: apply_total,
            message: if processed == apply_total {
                "Organization applied successfully.".to_string()
            } else {
                format!("Applying {processed}/{apply_total} actions...")
            },
        });
    }

    emit_organize_progress(EmitProgressParams {
        app: &ctx.app,
        state,
        session_id: &ctx.session_id,
        root_path: &ctx.root_path,
        phase: OrganizeProgressPhase::Done,
        processed: apply_total,
        total: apply_total,
        message: "Organization applied successfully.".into(),
    });

    let applied = parsed_actions.len();
    let summary = if skipped == 0 && warnings.is_empty() {
        format!("Applied {applied} organize actions.")
    } else {
        format!(
            "Applied {applied} organize actions, skipped {skipped} (missing files), {} warning(s).",
            warnings.len()
        )
    };
    Ok((summary, applied, warnings.len()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::IndexingProgressState;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64};
    use std::sync::{Arc, Mutex, RwLock};

    fn test_state() -> AppState {
        AppState {
            db: Mutex::new(rusqlite::Connection::open_in_memory().unwrap()),
            db_path: PathBuf::new(),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            file_operation_cancel_flags: Mutex::new(HashMap::new()),
            organize_cancel_flags: Mutex::new(HashMap::new()),
            organize_status: Mutex::new(HashMap::new()),
            organize_progress_sequence: AtomicU64::new(0),
            watcher_handle: Mutex::new(None),
            indexing_status: Arc::new(Mutex::new(IndexingProgressState {
                processed: 0,
                total: 0,
                status: "done".to_string(),
            })),
            last_user_interaction_at: Arc::new(AtomicI64::new(0)),
            permission_policy_cache: RwLock::new(None),
            permission_policy_version: AtomicU64::new(1),
        }
    }

    fn temp_test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "frogger_organize_pipeline_{name}_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn retryable_error_detection_excludes_permanent_errors() {
        assert!(!is_retryable_chunk_error(
            "Unauthorized. Check your authorization key."
        ));
        assert!(!is_retryable_chunk_error(
            "Bad request. Check your request parameters."
        ));
        assert!(!is_retryable_chunk_error("No API key configured"));
    }

    #[test]
    fn retryable_error_detection_includes_transient_errors() {
        assert!(is_retryable_chunk_error(
            "organization chunk timed out during planning"
        ));
        assert!(is_retryable_chunk_error(
            "Claude API error: Failed to send request"
        ));
        assert!(is_retryable_chunk_error(
            "Unexpected status code: {\"type\":\"rate_limit_error\"}"
        ));
        assert!(is_retryable_chunk_error(
            "HTTP status 429: too many requests"
        ));
    }

    #[test]
    fn retryable_error_detection_respects_http_status_classes() {
        assert!(!is_retryable_chunk_error("HTTP status 404: not found"));
        assert!(!is_retryable_chunk_error("HTTP status 401: unauthorized"));
        assert!(is_retryable_chunk_error("HTTP status 500: internal error"));
        assert!(is_retryable_chunk_error("HTTP status 529: overloaded"));
    }

    #[test]
    fn backoff_duration_is_positive_and_capped() {
        let d1 = retry_backoff_duration(1);
        let d6 = retry_backoff_duration(6);
        let d10 = retry_backoff_duration(10);

        assert!(d1 >= Duration::from_millis(250));
        assert!(d6 >= Duration::from_millis(5_000));
        assert!(d10 <= Duration::from_millis(5_249));
    }

    #[test]
    fn organize_error_kind_detects_db_lock_messages() {
        assert_eq!(
            organize_error_kind("Database error: database is locked"),
            "db_locked"
        );
        assert_eq!(
            organize_error_kind("Database error: database table is locked"),
            "db_locked"
        );
    }

    #[test]
    fn build_organize_audit_summary_includes_compact_error_context() {
        let summary = build_organize_audit_summary(
            "execute",
            "error",
            "session-123",
            Some(7),
            Some(1),
            Some("db_locked"),
            Some("Database error: database is locked\nwhile confirming plan"),
        );

        assert!(summary.contains("operation=execute"));
        assert!(summary.contains("outcome=error"));
        assert!(summary.contains("session=session-123"));
        assert!(summary.contains("actions=7"));
        assert!(summary.contains("warnings=1"));
        assert!(summary.contains("error_kind=db_locked"));
        assert!(summary.contains("error=Database error: database is locked while confirming plan"));
    }

    #[test]
    fn planning_progress_percent_bands_are_stable() {
        assert_eq!(planning_percent_first_pass(0, 10), 0);
        assert_eq!(planning_percent_first_pass(10, 10), 80);
        assert_eq!(planning_percent_refinement(0, 4), 80);
        assert_eq!(planning_percent_refinement(4, 4), 95);
    }

    #[test]
    fn record_progress_keeps_combined_percent_monotonic() {
        let state = test_state();

        let first = record_organize_progress(
            &state,
            "session-1",
            "/tmp",
            OrganizeProgressPhase::Planning,
            20,
            100,
            "Planning".to_string(),
        )
        .unwrap();
        let second = record_organize_progress(
            &state,
            "session-1",
            "/tmp",
            OrganizeProgressPhase::Indexing,
            90,
            100,
            "Indexing".to_string(),
        )
        .unwrap();

        assert_eq!(first.combined_percent, 25);
        assert_eq!(second.combined_percent, 25);
        assert!(second.sequence > first.sequence);
    }

    #[test]
    fn record_progress_stops_after_terminal_phase() {
        let state = test_state();

        let terminal = record_organize_progress(
            &state,
            "session-1",
            "/tmp",
            OrganizeProgressPhase::Cancelled,
            0,
            100,
            "Cancelled".to_string(),
        )
        .unwrap();
        let stale = record_organize_progress(
            &state,
            "session-1",
            "/tmp",
            OrganizeProgressPhase::Indexing,
            90,
            100,
            "Late indexing update".to_string(),
        );

        assert!(stale.is_none());
        let latest = state.get_organize_status("session-1").unwrap();
        assert_eq!(latest.phase, OrganizeProgressPhase::Cancelled);
        assert_eq!(latest.sequence, terminal.sequence);
    }

    #[test]
    fn filter_missing_sources_skips_missing_and_keeps_valid() {
        let dir = temp_test_dir("filter_missing");
        let existing = dir.join("exists.txt");
        std::fs::write(&existing, "ok").unwrap();
        let missing = dir.join("gone.txt").to_string_lossy().to_string();
        let existing_str = existing.to_string_lossy().to_string();

        let action = OrganizeExecutableAction::MoveFiles {
            sources: vec![existing_str.clone(), missing.clone()],
            dest_dir: dir.to_string_lossy().to_string(),
        };

        let mut warnings = Vec::new();
        let result = filter_missing_sources(action, 2, 5, &mut warnings);
        assert!(result.is_some());
        if let Some(OrganizeExecutableAction::MoveFiles { sources, .. }) = &result {
            assert_eq!(sources.len(), 1);
            assert_eq!(sources[0], existing_str);
        }
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("3/5"));
        assert!(warnings[0].contains(&missing));

        let action_all_missing = OrganizeExecutableAction::MoveFiles {
            sources: vec![missing.clone()],
            dest_dir: dir.to_string_lossy().to_string(),
        };
        let mut w2 = Vec::new();
        assert!(filter_missing_sources(action_all_missing, 0, 1, &mut w2).is_none());

        let _ = std::fs::remove_dir_all(dir);
    }
}
