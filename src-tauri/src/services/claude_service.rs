use std::sync::{Arc, Mutex};

use anthropic_sdk::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::error::AppError;

const MODEL: &str = "claude-sonnet-4-5-20250929";
const MAX_TOKENS: i32 = 4096;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct StreamChunk {
    pub chunk: String,
    pub done: bool,
}

pub fn build_system_prompt(
    current_dir: &str,
    selected_files: &[String],
    visible_files: &[String],
) -> String {
    let mut prompt = format!(
        "You are Frogger, an AI assistant embedded in a file manager app.\n\
         Current directory: {current_dir}"
    );
    if !selected_files.is_empty() {
        prompt.push_str(&format!("\nSelected files: {}", selected_files.join(", ")));
    }
    if !visible_files.is_empty() {
        let display: Vec<&str> = visible_files.iter().take(100).map(|s| s.as_str()).collect();
        prompt.push_str(&format!("\nVisible files: {}", display.join(", ")));
        if visible_files.len() > 100 {
            prompt.push_str(&format!(" ...and {} more", visible_files.len() - 100));
        }
    }
    prompt.push_str(
        "\n\nHelp with file organization, searching, and management. Be concise and action-oriented.\n\n\
         When performing file operations, respond with action blocks:\n\
         ```action\n{\"tool\": \"<tool_name>\", \"args\": {<arguments>}}\n```\n\n\
         You may include multiple action blocks in one response for batch operations.\n\n\
         Available tools:\n\
         - move_files: {\"sources\": [\"path1\", ...], \"dest_dir\": \"path\"}\n\
         - copy_files: {\"sources\": [\"path1\", ...], \"dest_dir\": \"path\"}\n\
         - rename_file: {\"source\": \"path\", \"destination\": \"path\"}\n\
         - delete_files: {\"paths\": [\"path1\", ...]}\n\
         - create_directory: {\"path\": \"path\"}\n\
         - batch_rename: {\"directory\": \"path\", \"pattern\": \"description of rename pattern\"}\n\
         For batch_rename: respond ONLY with individual rename_file action blocks. No explanatory text.\n\n\
         Always use absolute paths. The user will be asked to confirm before execution.",
    );
    prompt
}

pub async fn send_message(
    api_key: &str,
    messages: &[ChatMessage],
    system_prompt: &str,
    app: &AppHandle,
    temperature: Option<f32>,
    max_tokens: Option<i32>,
    stream: bool,
) -> Result<String, AppError> {
    let json_messages: Vec<serde_json::Value> = messages
        .iter()
        .map(|m| json!({"role": m.role, "content": m.content}))
        .collect();

    let request = Client::new()
        .auth(api_key)
        .model(MODEL)
        .system(system_prompt)
        .messages(&json!(json_messages))
        .max_tokens(max_tokens.unwrap_or(MAX_TOKENS))
        .temperature(temperature.unwrap_or(0.0))
        .stream(stream)
        .build()
        .map_err(|e| AppError::General(format!("Failed to build request: {e}")).capture())?;

    let accumulated = Arc::new(Mutex::new(String::new()));
    let acc_clone = accumulated.clone();
    let app_clone = app.clone();
    let do_stream = stream;

    request
        .execute(move |text| {
            let acc = acc_clone.clone();
            let app = app_clone.clone();
            async move {
                if let Ok(mut buf) = acc.lock() {
                    buf.push_str(&text);
                }
                if do_stream && !text.is_empty() {
                    let _ = app.emit(
                        "chat-stream",
                        StreamChunk {
                            chunk: text,
                            done: false,
                        },
                    );
                }
            }
        })
        .await
        .map_err(|e| AppError::General(format!("Claude API error: {e}")).capture())?;

    let full_response = accumulated
        .lock()
        .map_err(|e| AppError::General(format!("Failed to read response: {e}")))?
        .clone();

    eprintln!(
        "[claude] Response ({} chars): {}",
        full_response.len(),
        &full_response[..full_response.len().min(500)]
    );

    if do_stream {
        let _ = app.emit(
            "chat-stream",
            StreamChunk {
                chunk: String::new(),
                done: true,
            },
        );
    }

    Ok(full_response)
}
