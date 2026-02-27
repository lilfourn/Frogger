use tauri::{command, AppHandle, State};

use crate::commands::organize_pipeline::{
    self, EmitProgressParams, OrganizeTelemetryEvent, PipelineCtx,
};
use crate::data::repository::{self, ChatRecord};
use crate::error::AppError;
use crate::services::claude_service::{self, ChatMessage};
use crate::services::permission_service::{self, PermissionCapability};
use crate::state::{AppState, OrganizeProgressPhase, OrganizeProgressState};

const MAX_CONTEXT_MESSAGES: usize = 40;

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

// ---------------------------------------------------------------------------
// Organize commands — thin wrappers around organize_pipeline
// ---------------------------------------------------------------------------

#[command]
pub async fn send_organize_plan(
    current_dir: String,
    allow_once: Option<bool>,
    organize_session_id: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let _allow_once = allow_once.unwrap_or(false);
    let organize_session_id =
        organize_session_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let api_key = get_api_key().map_err(|e| e.capture())?;
    let cancel_flag = state.reset_organize_cancel_flag(&organize_session_id);
    state.clear_organize_status(&organize_session_id);

    let ctx = PipelineCtx {
        app: app.clone(),
        session_id: organize_session_id.clone(),
        root_path: current_dir.clone(),
        api_key,
        cancel_flag: cancel_flag.clone(),
    };

    let result = organize_pipeline::run_plan_pipeline(&ctx, state.inner()).await;

    let output = match result {
        Ok(plan_json) => Ok(plan_json),
        Err(err) => {
            if organize_pipeline::is_organize_cancelled(cancel_flag.as_ref())
                || err.to_string().contains("cancelled")
            {
                organize_pipeline::emit_organize_progress(EmitProgressParams {
                    app: &app,
                    state: state.inner(),
                    session_id: &organize_session_id,
                    root_path: &current_dir,
                    phase: OrganizeProgressPhase::Cancelled,
                    processed: 0,
                    total: 100,
                    message: "Organization cancelled.".into(),
                });
            } else {
                organize_pipeline::emit_organize_progress(EmitProgressParams {
                    app: &app,
                    state: state.inner(),
                    session_id: &organize_session_id,
                    root_path: &current_dir,
                    phase: OrganizeProgressPhase::Error,
                    processed: 0,
                    total: 100,
                    message: format!("Planning failed: {err}"),
                });
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

    let ctx = PipelineCtx {
        app: app.clone(),
        session_id: organize_session_id.clone(),
        root_path: current_dir.clone(),
        api_key: String::new(),
        cancel_flag: cancel_flag.clone(),
    };

    organize_pipeline::emit_organize_progress(EmitProgressParams {
        app: &app,
        state: state.inner(),
        session_id: &organize_session_id,
        root_path: &current_dir,
        phase: OrganizeProgressPhase::Applying,
        processed: 0,
        total: 1,
        message: "Preparing file operations...".into(),
    });

    let result = organize_pipeline::run_execute_pipeline(&ctx, state.inner(), &plan_json, allow_once);

    let output = match result {
        Ok((response, action_count, warning_count)) => {
            organize_pipeline::emit_organize_progress(EmitProgressParams {
                app: &app,
                state: state.inner(),
                session_id: &organize_session_id,
                root_path: &current_dir,
                phase: OrganizeProgressPhase::Applying,
                processed: 0,
                total: 1,
                message: "Action preview ready. Approve to apply changes.".into(),
            });
            organize_pipeline::record_organize_telemetry(
                state.inner(),
                OrganizeTelemetryEvent {
                    endpoint: "organize/execute",
                    operation: "execute",
                    outcome: "success",
                    session_id: &organize_session_id,
                    action_count: Some(action_count),
                    warning_count: Some(warning_count),
                    error_kind: None,
                    error_message: None,
                },
            );
            Ok(response)
        }
        Err(err) => {
            let err_text = err.to_string();
            let cancelled = organize_pipeline::is_organize_cancelled(cancel_flag.as_ref())
                || err_text.contains("cancelled");
            if cancelled {
                organize_pipeline::emit_organize_progress(EmitProgressParams {
                    app: &app,
                    state: state.inner(),
                    session_id: &organize_session_id,
                    root_path: &current_dir,
                    phase: OrganizeProgressPhase::Cancelled,
                    processed: 0,
                    total: 100,
                    message: "Organization cancelled.".into(),
                });
                organize_pipeline::record_organize_telemetry(
                    state.inner(),
                    OrganizeTelemetryEvent {
                        endpoint: "organize/execute",
                        operation: "execute",
                        outcome: "cancelled",
                        session_id: &organize_session_id,
                        action_count: None,
                        warning_count: None,
                        error_kind: Some("cancelled"),
                        error_message: Some(&err_text),
                    },
                );
            } else {
                let err_kind = organize_pipeline::organize_error_kind(&err_text);
                organize_pipeline::emit_organize_progress(EmitProgressParams {
                    app: &app,
                    state: state.inner(),
                    session_id: &organize_session_id,
                    root_path: &current_dir,
                    phase: OrganizeProgressPhase::Error,
                    processed: 0,
                    total: 100,
                    message: format!("Execution failed: {err}"),
                });
                organize_pipeline::record_organize_telemetry(
                    state.inner(),
                    OrganizeTelemetryEvent {
                        endpoint: "organize/execute",
                        operation: "execute",
                        outcome: "error",
                        session_id: &organize_session_id,
                        action_count: None,
                        warning_count: None,
                        error_kind: Some(err_kind),
                        error_message: Some(&err_text),
                    },
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

    let ctx = PipelineCtx {
        app: app.clone(),
        session_id: organize_session_id.clone(),
        root_path: current_dir.clone(),
        api_key: String::new(),
        cancel_flag: cancel_flag.clone(),
    };

    let result = organize_pipeline::run_apply_pipeline(&ctx, state.inner(), &plan_json, allow_once);

    let output = match result {
        Ok((summary, total, warning_count)) => {
            organize_pipeline::record_organize_telemetry(
                state.inner(),
                OrganizeTelemetryEvent {
                    endpoint: "organize/apply",
                    operation: "apply",
                    outcome: "success",
                    session_id: &organize_session_id,
                    action_count: Some(total),
                    warning_count: Some(warning_count),
                    error_kind: None,
                    error_message: None,
                },
            );
            Ok(summary)
        }
        Err(err) => {
            let err_text = err.to_string();
            let cancelled = organize_pipeline::is_organize_cancelled(cancel_flag.as_ref())
                || err_text.contains("cancelled");
            if cancelled {
                organize_pipeline::emit_organize_progress(EmitProgressParams {
                    app: &app,
                    state: state.inner(),
                    session_id: &organize_session_id,
                    root_path: &current_dir,
                    phase: OrganizeProgressPhase::Cancelled,
                    processed: 0,
                    total: 100,
                    message: "Organization cancelled.".into(),
                });
                organize_pipeline::record_organize_telemetry(
                    state.inner(),
                    OrganizeTelemetryEvent {
                        endpoint: "organize/apply",
                        operation: "apply",
                        outcome: "cancelled",
                        session_id: &organize_session_id,
                        action_count: None,
                        warning_count: None,
                        error_kind: Some("cancelled"),
                        error_message: Some(&err_text),
                    },
                );
            } else {
                let err_kind = organize_pipeline::organize_error_kind(&err_text);
                organize_pipeline::emit_organize_progress(EmitProgressParams {
                    app: &app,
                    state: state.inner(),
                    session_id: &organize_session_id,
                    root_path: &current_dir,
                    phase: OrganizeProgressPhase::Error,
                    processed: 0,
                    total: 100,
                    message: format!("Apply failed: {err}"),
                });
                organize_pipeline::record_organize_telemetry(
                    state.inner(),
                    OrganizeTelemetryEvent {
                        endpoint: "organize/apply",
                        operation: "apply",
                        outcome: "error",
                        session_id: &organize_session_id,
                        action_count: None,
                        warning_count: None,
                        error_kind: Some(err_kind),
                        error_message: Some(&err_text),
                    },
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
    organize_pipeline::emit_organize_progress(EmitProgressParams {
        app: &app,
        state: state.inner(),
        session_id: &organize_session_id,
        root_path: &current_dir,
        phase: OrganizeProgressPhase::Cancelled,
        processed: 0,
        total: 100,
        message: "Cancellation requested.".into(),
    });
}

#[command]
pub fn get_organize_status(
    organize_session_id: String,
    state: State<'_, AppState>,
) -> Result<Option<OrganizeProgressState>, AppError> {
    Ok(state.get_organize_status(&organize_session_id))
}

#[command]
pub fn new_chat_session() -> String {
    uuid::Uuid::new_v4().to_string()
}
