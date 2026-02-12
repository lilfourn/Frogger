use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{command, AppHandle, Emitter, State};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::data::repository::{self, ChatRecord};
use crate::error::AppError;
use crate::services::claude_service::{self, ChatMessage};
use crate::services::file_service;
use crate::services::indexing_service;
use crate::services::organize_service;
use crate::services::permission_service::{self, PermissionCapability};
use crate::state::{AppState, PermissionPolicyCacheEntry};

const MAX_CONTEXT_MESSAGES: usize = 40;
const ORGANIZE_MAX_FILES: usize = organize_service::DEFAULT_MAX_FILES;
const ORGANIZE_CHUNK_SIZE: usize = organize_service::DEFAULT_CHUNK_SIZE;
const INDEXING_WEIGHT: f64 = 0.10;
const PLANNING_WEIGHT: f64 = 0.75;
const APPLYING_WEIGHT: f64 = 0.15;
const CHUNK_REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
const ORGANIZE_PARALLEL_CHUNK_REQUESTS: usize = 3;
const ORGANIZE_CHUNK_RETRY_ATTEMPTS: usize = 5;
const ORGANIZE_REFINEMENT_CHUNK_SIZE: usize = 120;
const PLANNING_FIRST_PASS_MAX_PERCENT: usize = 80;
const PLANNING_REFINEMENT_MAX_PERCENT: usize = 95;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrganizeProgressEvent {
    session_id: String,
    root_path: String,
    phase: String,
    processed: usize,
    total: usize,
    percent: usize,
    combined_percent: usize,
    message: String,
}

fn phase_percent(processed: usize, total: usize) -> usize {
    if total == 0 {
        return 0;
    }
    ((processed as f64 / total as f64) * 100.0)
        .round()
        .clamp(0.0, 100.0) as usize
}

fn combined_percent_for_phase(phase: &str, percent: usize) -> usize {
    match phase {
        "indexing" => ((percent as f64) * INDEXING_WEIGHT).round() as usize,
        "planning" => (10.0 + (percent as f64) * PLANNING_WEIGHT).round() as usize,
        "applying" => (85.0 + (percent as f64) * APPLYING_WEIGHT).round() as usize,
        "done" => 100,
        _ => percent,
    }
    .min(100)
}

fn emit_organize_progress(
    app: &AppHandle,
    session_id: &str,
    root_path: &str,
    phase: &str,
    processed: usize,
    total: usize,
    message: impl Into<String>,
) {
    let percent = phase_percent(processed, total);
    let payload = OrganizeProgressEvent {
        session_id: session_id.to_string(),
        root_path: root_path.to_string(),
        phase: phase.to_string(),
        processed,
        total,
        percent,
        combined_percent: combined_percent_for_phase(phase, percent),
        message: message.into(),
    };
    let _ = app.emit("organize-progress", payload);
}

fn is_organize_cancelled(cancel_flag: &AtomicBool) -> bool {
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
    session_id: &str,
    root_path: &str,
    planning_percent: usize,
    message: impl Into<String>,
) {
    emit_organize_progress(
        app,
        session_id,
        root_path,
        "planning",
        planning_percent.min(100),
        100,
        message,
    );
}

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
            Some(2048),
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

#[derive(Debug, Clone)]
struct ChunkClassificationJob {
    idx: usize,
    prompt: String,
    chunk_paths: HashSet<String>,
}

#[derive(Debug, Clone)]
struct ChunkClassificationResult {
    idx: usize,
    used_fallback: bool,
    chunk_paths: HashSet<String>,
    map: HashMap<String, Vec<String>>,
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

            let mut used_fallback = false;
            let map = match organize_service::parse_chunk_plan(&response, &job.chunk_paths) {
                Ok(parsed) => parsed,
                Err(parse_err) => {
                    used_fallback = true;
                    sentry::capture_message(
                        &format!(
                            "Organize chunk parse failed: {parse_err}. Falling back to 'other'."
                        ),
                        sentry::Level::Warning,
                    );
                    let mut fallback = HashMap::new();
                    let mut sorted_paths = job.chunk_paths.iter().cloned().collect::<Vec<_>>();
                    sorted_paths.sort();
                    fallback.insert("other".to_string(), sorted_paths);
                    fallback
                }
            };

            Ok(ChunkClassificationResult {
                idx: job.idx,
                used_fallback,
                chunk_paths: job.chunk_paths,
                map,
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

fn get_api_key() -> Result<String, AppError> {
    let entry = keyring::Entry::new("frogger", "api_key")
        .map_err(|e| AppError::General(format!("Keychain error: {e}")))?;
    entry
        .get_password()
        .map_err(|_| AppError::General("No API key configured".to_string()))
}

#[command]
pub async fn send_chat(
    message: String,
    session_id: String,
    current_dir: String,
    selected_files: Vec<String>,
    allow_once: Option<bool>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let allow_once = allow_once.unwrap_or(false);
    let api_key = get_api_key().map_err(|e| e.capture())?;

    {
        let conn = state
            .db
            .lock()
            .map_err(|e| AppError::General(e.to_string()))?;
        let policy = permission_service::load_policy_cache_entry(&conn, &state)?;
        permission_service::enforce_with_cached_policy(
            &policy,
            &current_dir,
            PermissionCapability::ContentScan,
            allow_once,
        )?;
        for path in &selected_files {
            permission_service::enforce_with_cached_policy(
                &policy,
                path,
                PermissionCapability::ContentScan,
                allow_once,
            )?;
        }
        repository::insert_chat_message(&conn, &session_id, "user", &message)?;
    }

    let history = {
        let conn = state
            .db
            .lock()
            .map_err(|e| AppError::General(e.to_string()))?;
        repository::get_chat_messages(&conn, &session_id)?
    };

    let messages: Vec<ChatMessage> = history
        .iter()
        .rev()
        .take(MAX_CONTEXT_MESSAGES)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|r| ChatMessage {
            role: r.role.clone(),
            content: r.content.clone(),
        })
        .collect();

    let visible_files: Vec<String> = std::fs::read_dir(&current_dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();

    let system = claude_service::build_system_prompt(&current_dir, &selected_files, &visible_files);

    let response =
        claude_service::send_message(&api_key, &messages, &system, &app, None, None, true)
            .await
            .map_err(|e| {
                sentry::configure_scope(|scope| {
                    scope.set_extra("user_message", serde_json::Value::String(message.clone()));
                });
                e.capture()
            })?;

    {
        let conn = state
            .db
            .lock()
            .map_err(|e| AppError::General(e.to_string()))?;
        repository::insert_chat_message(&conn, &session_id, "assistant", &response)?;

        let summary = format!(
            "chat: {} files in context, {} msg history",
            selected_files.len(),
            messages.len()
        );
        let _ = repository::insert_audit_log(&conn, "claude/messages", Some(&summary), None, None);
    }

    Ok(response)
}

#[command]
pub fn get_chat_history(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ChatRecord>, AppError> {
    let conn = state
        .db
        .lock()
        .map_err(|e| AppError::General(e.to_string()))?;
    repository::get_chat_messages(&conn, &session_id)
}

#[command]
pub fn clear_chat_history(session_id: String, state: State<'_, AppState>) -> Result<(), AppError> {
    let conn = state
        .db
        .lock()
        .map_err(|e| AppError::General(e.to_string()))?;
    repository::delete_chat_session(&conn, &session_id)
}

#[command]
pub async fn send_organize_plan(
    current_dir: String,
    allow_once: Option<bool>,
    organize_session_id: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let allow_once = allow_once.unwrap_or(false);
    let organize_session_id =
        organize_session_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let api_key = get_api_key().map_err(|e| e.capture())?;
    let cancel_flag = state.reset_organize_cancel_flag(&organize_session_id);

    let result: Result<String, AppError> = async {
        let indexing_total_hint = 100usize;
        emit_organize_progress(
            &app,
            &organize_session_id,
            &current_dir,
            "indexing",
            0,
            indexing_total_hint,
            "Indexing directory tree...",
        );

        let (manifest, skipped_hidden, skipped_already_organized) = {
            let conn = state
                .db
                .lock()
                .map_err(|e| AppError::General(e.to_string()))?;
            let policy = permission_service::load_policy_cache_entry(&conn, &state)?;

            // Organize planning reads indexed content and requires subtree index freshness.
            permission_service::enforce_with_cached_policy(
                &policy,
                &current_dir,
                PermissionCapability::ContentScan,
                allow_once,
            )?;
            permission_service::enforce_with_cached_policy(
                &policy,
                &current_dir,
                PermissionCapability::Indexing,
                allow_once,
            )?;

            let cancelled = indexing_service::scan_directory_deep_with_progress_cancel(
                &conn,
                &current_dir,
                allow_once,
                cancel_flag.as_ref(),
                |processed, total| {
                    emit_organize_progress(
                        &app,
                        &organize_session_id,
                        &current_dir,
                        "indexing",
                        processed,
                        total,
                        format!("Indexing files {processed}/{total}"),
                    );
                },
            );
            if cancelled || is_organize_cancelled(cancel_flag.as_ref()) {
                return Err(AppError::General("organization cancelled".to_string()));
            }

            organize_service::collect_indexed_manifest(
                &conn,
                &current_dir,
                false,
                ORGANIZE_MAX_FILES,
            )?
        };

        if manifest.is_empty() {
            return Err(AppError::General(
                "No indexed files found for this directory. Try indexing and retry.".to_string(),
            ));
        }

        let tree_summary = organize_service::build_tree_summary(&manifest);
        let chunks = organize_service::chunk_manifest(&manifest, ORGANIZE_CHUNK_SIZE);
        emit_planning_progress(
            &app,
            &organize_session_id,
            &current_dir,
            2,
            format!(
                "Classifying {} files in {} chunks...",
                manifest.len(),
                chunks.len()
            ),
        );

        let first_pass_jobs = chunks
            .iter()
            .enumerate()
            .map(|(idx, chunk)| {
                let prompt = organize_service::build_chunk_prompt(
                    &current_dir,
                    idx + 1,
                    chunks.len(),
                    &tree_summary,
                    chunk,
                )?;
                let chunk_paths = chunk
                    .iter()
                    .map(|f| f.relative_path.clone())
                    .collect::<HashSet<_>>();
                Ok(ChunkClassificationJob {
                    idx,
                    prompt,
                    chunk_paths,
                })
            })
            .collect::<Result<Vec<_>, AppError>>()?;

        let first_pass_results = classify_chunk_batch(
            first_pass_jobs,
            &api_key,
            &app,
            cancel_flag.clone(),
            &current_dir,
            "Classify this file chunk into allowed categories.",
            |completed, total, latest_idx| {
                let planning_percent = planning_percent_first_pass(completed, total);
                emit_planning_progress(
                    &app,
                    &organize_session_id,
                    &current_dir,
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

        if is_organize_cancelled(cancel_flag.as_ref()) {
            return Err(AppError::General("organization cancelled".to_string()));
        }

        let mut merged: HashMap<String, Vec<String>> = HashMap::new();
        let mut fallback_paths: HashSet<String> = HashSet::new();
        for result in &first_pass_results {
            if result.used_fallback {
                fallback_paths.extend(result.chunk_paths.iter().cloned());
            }
            for (folder, files) in &result.map {
                merged
                    .entry(folder.clone())
                    .or_default()
                    .extend(files.clone());
            }
        }

        emit_planning_progress(
            &app,
            &organize_session_id,
            &current_dir,
            PLANNING_FIRST_PASS_MAX_PERCENT,
            "Classification complete. Checking for ambiguous files...",
        );

        let mut refinement_chunk_count = 0usize;
        let mut refinement_candidates = fallback_paths;
        if let Some(other_files) = merged.get("other") {
            refinement_candidates.extend(other_files.iter().cloned());
        }

        if !refinement_candidates.is_empty() {
            let manifest_index = manifest
                .iter()
                .map(|f| (f.relative_path.clone(), f.clone()))
                .collect::<HashMap<_, _>>();

            let mut refinement_files = refinement_candidates
                .into_iter()
                .filter_map(|rel| manifest_index.get(&rel).cloned())
                .collect::<Vec<_>>();
            refinement_files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
            refinement_files.dedup_by(|a, b| a.relative_path == b.relative_path);

            let refinement_chunks =
                organize_service::chunk_manifest(&refinement_files, ORGANIZE_REFINEMENT_CHUNK_SIZE);
            refinement_chunk_count = refinement_chunks.len();

            if refinement_chunk_count > 0 {
                emit_planning_progress(
                    &app,
                    &organize_session_id,
                    &current_dir,
                    PLANNING_FIRST_PASS_MAX_PERCENT,
                    format!(
                        "Refining ambiguous assignments across {} chunk(s)...",
                        refinement_chunk_count
                    ),
                );

                let current_assignments = organize_service::category_assignments(&merged);
                let refinement_jobs = refinement_chunks
                    .iter()
                    .enumerate()
                    .map(|(idx, chunk)| {
                        let prompt = organize_service::build_refinement_chunk_prompt(
                            &current_dir,
                            idx + 1,
                            refinement_chunks.len(),
                            &tree_summary,
                            chunk,
                            &current_assignments,
                        )?;
                        let chunk_paths = chunk
                            .iter()
                            .map(|f| f.relative_path.clone())
                            .collect::<HashSet<_>>();
                        Ok(ChunkClassificationJob {
                            idx,
                            prompt,
                            chunk_paths,
                        })
                    })
                    .collect::<Result<Vec<_>, AppError>>()?;

                let refinement_results = classify_chunk_batch(
                    refinement_jobs,
                    &api_key,
                    &app,
                    cancel_flag.clone(),
                    &current_dir,
                    "Refine ambiguous file classifications with high confidence.",
                    |completed, total, _| {
                        let planning_percent = planning_percent_refinement(completed, total);
                        emit_planning_progress(
                            &app,
                            &organize_session_id,
                            &current_dir,
                            planning_percent,
                            format!("Refined ambiguous chunk {completed}/{total}"),
                        );
                    },
                )
                .await?;

                let mut refinement_merged: HashMap<String, Vec<String>> = HashMap::new();
                for result in refinement_results {
                    for (folder, files) in result.map {
                        refinement_merged.entry(folder).or_default().extend(files);
                    }
                }

                merged = organize_service::reconcile_with_refinement(merged, refinement_merged);
            }
        }

        emit_planning_progress(
            &app,
            &organize_session_id,
            &current_dir,
            PLANNING_REFINEMENT_MAX_PERCENT,
            "Finalizing organization plan...",
        );

        let plan = organize_service::build_plan_document(
            merged,
            organize_service::OrganizePlanStats {
                total_files: manifest.len(),
                indexed_files: manifest.len(),
                skipped_hidden,
                skipped_already_organized,
                chunks: chunks.len() + refinement_chunk_count,
            },
        );

        emit_organize_progress(
            &app,
            &organize_session_id,
            &current_dir,
            "planning",
            100,
            100,
            "Plan ready for review.",
        );

        organize_service::plan_to_json_block(&plan)
    }
    .await;

    let output = match result {
        Ok(plan_json) => Ok(plan_json),
        Err(err) => {
            if is_organize_cancelled(cancel_flag.as_ref()) || err.to_string().contains("cancelled")
            {
                emit_organize_progress(
                    &app,
                    &organize_session_id,
                    &current_dir,
                    "cancelled",
                    0,
                    100,
                    "Organization cancelled.",
                );
            } else {
                emit_organize_progress(
                    &app,
                    &organize_session_id,
                    &current_dir,
                    "error",
                    0,
                    100,
                    format!("Planning failed: {err}"),
                );
            }
            Err(err)
        }
    };
    state.clear_organize_cancel_flag(&organize_session_id);
    output
}

#[command]
pub fn send_organize_execute(
    current_dir: String,
    plan_json: String,
    allow_once: Option<bool>,
    organize_session_id: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let allow_once = allow_once.unwrap_or(false);
    let organize_session_id =
        organize_session_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let cancel_flag = state.reset_organize_cancel_flag(&organize_session_id);
    emit_organize_progress(
        &app,
        &organize_session_id,
        &current_dir,
        "applying",
        0,
        1,
        "Preparing file operations...",
    );

    let result: Result<String, AppError> = (|| {
        let existing_paths = {
            let conn = state
                .db
                .lock()
                .map_err(|e| AppError::General(e.to_string()))?;
            let policy = permission_service::load_policy_cache_entry(&conn, &state)?;
            permission_service::enforce_with_cached_policy(
                &policy,
                &current_dir,
                PermissionCapability::ContentScan,
                allow_once,
            )?;
            let (manifest, _, _) = organize_service::collect_indexed_manifest(
                &conn,
                &current_dir,
                true,
                ORGANIZE_MAX_FILES,
            )?;
            manifest
                .into_iter()
                .map(|file| file.absolute_path)
                .collect::<HashSet<String>>()
        };

        if is_organize_cancelled(cancel_flag.as_ref()) {
            return Err(AppError::General("organization cancelled".to_string()));
        }

        let payload = organize_service::extract_json_payload(&plan_json)
            .ok_or_else(|| AppError::General("Invalid organize plan payload".to_string()))?;
        let parsed_plan: organize_service::OrganizePlan = serde_json::from_str(&payload)?;

        let batch =
            organize_service::build_action_batch(&current_dir, &parsed_plan, &existing_paths);
        if is_organize_cancelled(cancel_flag.as_ref()) {
            return Err(AppError::General("organization cancelled".to_string()));
        }
        let response = organize_service::actions_to_blocks(&batch)?;
        Ok(response)
    })();

    let output = match result {
        Ok(response) => {
            emit_organize_progress(
                &app,
                &organize_session_id,
                &current_dir,
                "applying",
                0,
                1,
                "Action preview ready. Approve to apply changes.",
            );
            Ok(response)
        }
        Err(err) => {
            if is_organize_cancelled(cancel_flag.as_ref()) || err.to_string().contains("cancelled")
            {
                emit_organize_progress(
                    &app,
                    &organize_session_id,
                    &current_dir,
                    "cancelled",
                    0,
                    100,
                    "Organization cancelled.",
                );
            } else {
                emit_organize_progress(
                    &app,
                    &organize_session_id,
                    &current_dir,
                    "error",
                    0,
                    100,
                    format!("Execution failed: {err}"),
                );
            }
            Err(err)
        }
    };
    state.clear_organize_cancel_flag(&organize_session_id);
    output
}

#[command]
pub fn send_organize_apply(
    current_dir: String,
    plan_json: String,
    allow_once: Option<bool>,
    organize_session_id: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let allow_once = allow_once.unwrap_or(false);
    let organize_session_id =
        organize_session_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let cancel_flag = state.reset_organize_cancel_flag(&organize_session_id);

    let result: Result<String, AppError> = (|| {
        let (batch, warnings, policy) = {
            let conn = state
                .db
                .lock()
                .map_err(|e| AppError::General(e.to_string()))?;
            let policy = permission_service::load_policy_cache_entry(&conn, &state)?;
            permission_service::enforce_with_cached_policy(
                &policy,
                &current_dir,
                PermissionCapability::Modification,
                allow_once,
            )?;

            let (manifest, _, _) = organize_service::collect_indexed_manifest(
                &conn,
                &current_dir,
                true,
                ORGANIZE_MAX_FILES,
            )?;
            let existing_paths = manifest
                .into_iter()
                .map(|file| file.absolute_path)
                .collect::<HashSet<String>>();

            let payload = organize_service::extract_json_payload(&plan_json)
                .ok_or_else(|| AppError::General("Invalid organize plan payload".to_string()))?;
            let parsed_plan: organize_service::OrganizePlan = serde_json::from_str(&payload)?;
            let batch =
                organize_service::build_action_batch(&current_dir, &parsed_plan, &existing_paths);
            (batch.actions, batch.warnings, policy)
        };

        if batch.is_empty() {
            let warning_text = if warnings.is_empty() {
                "No safe actions were generated for apply.".to_string()
            } else {
                warnings.join("\n")
            };
            return Err(AppError::General(warning_text));
        }

        let total = batch.len();
        emit_organize_progress(
            &app,
            &organize_session_id,
            &current_dir,
            "applying",
            0,
            total,
            format!("Applying 0/{total} actions..."),
        );

        for (idx, raw_action) in batch.iter().enumerate() {
            if is_organize_cancelled(cancel_flag.as_ref()) {
                return Err(AppError::General("organization cancelled".to_string()));
            }

            let parsed_action = parse_organize_action(raw_action)?;
            enforce_organize_action(&policy, &parsed_action, allow_once)?;
            apply_organize_action(&parsed_action)?;

            let processed = idx + 1;
            emit_organize_progress(
                &app,
                &organize_session_id,
                &current_dir,
                "applying",
                processed,
                total,
                if processed == total {
                    "Organization applied successfully.".to_string()
                } else {
                    format!("Applying {processed}/{total} actions...")
                },
            );
        }

        emit_organize_progress(
            &app,
            &organize_session_id,
            &current_dir,
            "done",
            total,
            total,
            "Organization applied successfully.",
        );

        let summary = if warnings.is_empty() {
            format!("Applied {total} organize actions.")
        } else {
            format!(
                "Applied {total} organize actions with {} warning(s).",
                warnings.len()
            )
        };
        Ok(summary)
    })();

    let output = match result {
        Ok(summary) => Ok(summary),
        Err(err) => {
            if is_organize_cancelled(cancel_flag.as_ref()) || err.to_string().contains("cancelled")
            {
                emit_organize_progress(
                    &app,
                    &organize_session_id,
                    &current_dir,
                    "cancelled",
                    0,
                    100,
                    "Organization cancelled.",
                );
            } else {
                emit_organize_progress(
                    &app,
                    &organize_session_id,
                    &current_dir,
                    "error",
                    0,
                    100,
                    format!("Apply failed: {err}"),
                );
            }
            Err(err)
        }
    };
    state.clear_organize_cancel_flag(&organize_session_id);
    output
}

#[command]
pub fn cancel_organize(
    current_dir: String,
    organize_session_id: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) {
    state.mark_organize_cancelled(organize_session_id.as_deref());
    let organize_session_id =
        organize_session_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    emit_organize_progress(
        &app,
        &organize_session_id,
        &current_dir,
        "cancelled",
        0,
        100,
        "Cancellation requested.",
    );
}

#[command]
pub fn new_chat_session() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn planning_progress_percent_bands_are_stable() {
        assert_eq!(planning_percent_first_pass(0, 10), 0);
        assert_eq!(planning_percent_first_pass(10, 10), 80);
        assert_eq!(planning_percent_refinement(0, 4), 80);
        assert_eq!(planning_percent_refinement(4, 4), 95);
    }
}
