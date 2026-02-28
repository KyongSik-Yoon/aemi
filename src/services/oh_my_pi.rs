use std::process::Stdio;
use std::sync::mpsc::Sender;
use serde_json::Value;

pub use super::agent::{StreamMessage, CancelToken, AgentResponse};
use super::provider_common::{self, StreamingConfig};

// Generate resolve_binary_path(), get_binary_path(), is_cli_available(), debug_log() for "omp"
define_ai_service_helpers!("omp");

/// Check if oh-my-pi CLI is available
#[allow(dead_code)]
pub fn is_omp_available() -> bool {
    is_cli_available()
}

/// Execute a command using oh-my-pi CLI (non-streaming)
#[allow(dead_code)]
pub fn execute_command(
    prompt: &str,
    session_id: Option<&str>,
    working_dir: &str,
) -> AgentResponse {
    let mut args = vec![
        "--print".to_string(),
        "--mode".to_string(),
        "json".to_string(),
    ];

    // Session resume support
    if let Some(sid) = session_id {
        args.push("--resume".to_string());
        args.push(sid.to_string());
    }

    // Prompt as positional argument (last)
    args.push(prompt.to_string());

    let omp_bin = match get_binary_path() {
        Some(path) => path,
        None => {
            return AgentResponse {
                success: false,
                response: None,
                session_id: None,
                error: Some("oh-my-pi CLI (omp) not found. Is oh-my-pi installed?".to_string()),
            };
        }
    };

    let child = match Command::new(omp_bin)
        .args(&args)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            return AgentResponse {
                success: false,
                response: None,
                session_id: None,
                error: Some(format!("Failed to start oh-my-pi: {}. Is oh-my-pi (omp) installed?", e)),
            };
        }
    };

    // Wait for output — parse JSONL and extract last text
    match child.wait_with_output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                parse_omp_jsonl_output(&stdout)
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let err_msg = if stderr.is_empty() {
                    format!("Process exited with code {:?}", output.status.code())
                } else {
                    stderr
                };

                // If session not found, retry without --resume
                if session_id.is_some() && is_session_not_found_error(&err_msg) {
                    let retry_args = vec![
                        "--print".to_string(),
                        "--mode".to_string(),
                        "json".to_string(),
                        prompt.to_string(),
                    ];

                    if let Ok(retry_child) = Command::new(omp_bin)
                        .args(&retry_args)
                        .current_dir(working_dir)
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn()
                    {
                        if let Ok(retry_output) = retry_child.wait_with_output() {
                            if retry_output.status.success() {
                                let stdout = String::from_utf8_lossy(&retry_output.stdout).to_string();
                                return parse_omp_jsonl_output(&stdout);
                            }
                        }
                    }
                }

                AgentResponse {
                    success: false,
                    response: None,
                    session_id: None,
                    error: Some(err_msg),
                }
            }
        }
        Err(e) => AgentResponse {
            success: false,
            response: None,
            session_id: None,
            error: Some(format!("Failed to read output: {}", e)),
        },
    }
}

/// Extract text from either legacy string content or Pi-style rich content arrays.
///
/// Pi runner schema uses e.g.: `{"content":[{"type":"text","text":"Done."}]}`.
fn extract_text_from_content_value(content: &Value) -> String {
    fn collect(v: &Value, out: &mut String) {
        match v {
            Value::String(s) => out.push_str(s),
            Value::Array(arr) => {
                for item in arr {
                    collect(item, out);
                }
            }
            Value::Object(obj) => {
                // Common: {"type":"text","text":"..."}
                if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                    out.push_str(text);
                    return;
                }
                // Fallback: objects that nest content arrays
                if let Some(content) = obj.get("content") {
                    collect(content, out);
                }
            }
            _ => {}
        }
    }
    let mut out = String::new();
    collect(content, &mut out);
    out
}

fn extract_assistant_text_from_message(message: &Value) -> Option<String> {
    let role = message.get("role").and_then(|v| v.as_str()).unwrap_or("");
    if role != "assistant" {
        return None;
    }

    let content = message.get("content")?;
    let text = extract_text_from_content_value(content);
    if text.trim().is_empty() { None } else { Some(text) }
}

fn normalize_tool_name(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "bash" => "Bash".to_string(),
        "read" => "Read".to_string(),
        "write" => "Write".to_string(),
        "edit" => "Edit".to_string(),
        "grep" => "Grep".to_string(),
        "glob" => "Glob".to_string(),
        "websearch" | "web_search" => "WebSearch".to_string(),
        "webfetch" | "web_fetch" => "WebFetch".to_string(),
        "task" => "Task".to_string(),
        "taskoutput" | "task_output" => "TaskOutput".to_string(),
        "taskstop" | "task_stop" => "TaskStop".to_string(),
        _ => name.to_string(),
    }
}
fn format_pi_tool_args_for_display(tool_name: &str, args: &Value) -> String {
    // Map Pi runner tool args to the schema our formatter expects, where possible.
    // This is display-only (does not affect actual tool execution).
    if tool_name == "Read" {
        if let Some(obj) = args.as_object() {
            if let Some(path) = obj.get("path").and_then(|v| v.as_str()) {
                let mut mapped = serde_json::Map::new();
                mapped.insert("file_path".to_string(), Value::String(path.to_string()));
                if let Some(offset) = obj.get("offset") {
                    mapped.insert("offset".to_string(), offset.clone());
                }
                if let Some(limit) = obj.get("limit") {
                    mapped.insert("limit".to_string(), limit.clone());
                }
                return serde_json::to_string_pretty(&Value::Object(mapped)).unwrap_or_default();
            }
        }
    }

    match args {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        _ => serde_json::to_string_pretty(args).unwrap_or_default(),
    }
}
/// Parse oh-my-pi CLI JSONL output: extract session id and final assistant text.
///
/// Supports both legacy `omp` events (`type=message/tool_use/...`) and modern
/// Pi runner events (`type=session/message_end/tool_execution_*`).
#[allow(dead_code)]
fn parse_omp_jsonl_output(output: &str) -> AgentResponse {
    let mut session_id: Option<String> = None;
    let mut last_text: Option<String> = None;

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let Ok(json) = serde_json::from_str::<Value>(line) else {
            continue;
        };

        let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");

        // Capture session id. New Pi schema emits a `session` header line with `id`.
        if session_id.is_none() {
            session_id = match msg_type {
                "session" => json.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                _ => json.get("sessionId").and_then(|v| v.as_str()).map(|s| s.to_string()),
            };
        }

        match msg_type {
            // Modern Pi schema: final assistant message is on message_end (and repeated on turn_end/agent_end).
            "message_end" | "turn_end" => {
                if let Some(message) = json.get("message") {
                    if let Some(text) = extract_assistant_text_from_message(message) {
                        last_text = Some(text);
                    }
                }
            }
            "agent_end" => {
                if let Some(messages) = json.get("messages").and_then(|v| v.as_array()) {
                    for m in messages {
                        if let Some(text) = extract_assistant_text_from_message(m) {
                            last_text = Some(text);
                        }
                    }
                }
            }

            // Legacy schema: flat message event with `role` + string `content`.
            "message" => {
                let role = json.get("role").and_then(|v| v.as_str()).unwrap_or("");
                if role == "assistant" {
                    if let Some(content) = json.get("content") {
                        let text = extract_text_from_content_value(content);
                        if !text.trim().is_empty() {
                            last_text = Some(text);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let response = last_text.and_then(|t| if t.trim().is_empty() { None } else { Some(t) });
    AgentResponse {
        success: response.is_some(),
        response,
        session_id,
        error: None,
    }
}

/// Check if an error message indicates a session-not-found condition.
fn is_session_not_found_error(err: &str) -> bool {
    let lower = err.to_lowercase();
    lower.contains("session") && lower.contains("not found")
}

/// oh-my-pi custom JSON handler: extracts session id and sends Init.
fn handle_omp_json(
    json: &Value,
    sender: &Sender<StreamMessage>,
    state: &mut provider_common::StreamState,
) -> bool {
    let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");

    // Capture session id. New Pi schema emits: {"type":"session","id":"..."}.
    if state.session_id.is_none() {
        state.session_id = if msg_type == "session" {
            json.get("id").and_then(|v| v.as_str()).map(|s| s.to_string())
        } else {
            json.get("sessionId").and_then(|v| v.as_str()).map(|s| s.to_string())
        };
    }

    // Send Init on first event with a session id
    if !state.sent_init {
        if let Some(ref sid) = state.session_id {
            let _ = sender.send(StreamMessage::Init { session_id: sid.clone() });
            state.sent_init = true;
        }
    }

    if let Some(msg) = parse_stream_message(json) {
        return provider_common::handle_parsed_message(msg, sender, state);
    }
    true
}

/// Execute a command using oh-my-pi CLI with streaming output
pub fn execute_command_streaming(
    prompt: &str,
    session_id: Option<&str>,
    working_dir: &str,
    sender: Sender<StreamMessage>,
    system_prompt: Option<&str>,
    _allowed_tools: Option<&[String]>, // oh-my-pi manages tools internally
    cancel_token: Option<std::sync::Arc<CancelToken>>,
) -> Result<(), String> {
    debug_log(&format!("prompt_len: {} chars", prompt.len()));
    debug_log(&format!("session_id: {:?}", session_id));

    let effective_prompt = provider_common::build_effective_prompt(system_prompt, prompt);

    // Build args: omp --print --mode json [--resume <session_id>] "prompt"
    let mut args = vec![
        "--print".to_string(),
        "--mode".to_string(),
        "json".to_string(),
    ];

    // Session resume support
    if let Some(sid) = session_id {
        debug_log(&format!("Resuming session: {}", sid));
        args.push("--resume".to_string());
        args.push(sid.to_string());
    }

    // Prompt as positional argument (must be last)
    args.push(effective_prompt.clone());

    let binary_path = get_binary_path()
        .ok_or_else(|| {
            debug_log("ERROR: oh-my-pi CLI (omp) not found");
            "oh-my-pi CLI (omp) not found. Is oh-my-pi installed?".to_string()
        })?;

    let config = StreamingConfig {
        provider_name: "oh-my-pi",
        binary_path,
        args: &args,
        working_dir,
        env_vars: &[],
        env_remove: &[],
        stdin_data: None,
        send_synthetic_init: false,
    };

    // Clone sender and cancel_token for potential retry on session-not-found
    let sender_retry = sender.clone();
    let cancel_retry = cancel_token.clone();

    let result = provider_common::run_streaming(
        &config,
        sender,
        cancel_token,
        handle_omp_json,
    );

    // If session resume failed with "session not found", retry without --resume
    if let Err(ref e) = result {
        if session_id.is_some() && is_session_not_found_error(e) {
            debug_log(&format!("Session not found, retrying without --resume: {}", e));

            let retry_args = vec![
                "--print".to_string(),
                "--mode".to_string(),
                "json".to_string(),
                effective_prompt,
            ];

            let retry_config = StreamingConfig {
                provider_name: "oh-my-pi",
                binary_path,
                args: &retry_args,
                working_dir,
                env_vars: &[],
                env_remove: &[],
                stdin_data: None,
                send_synthetic_init: false,
            };

            return provider_common::run_streaming(
                &retry_config,
                sender_retry,
                cancel_retry,
                handle_omp_json,
            );
        }
    }

    result
}

/// Parse an oh-my-pi JSONL event into a StreamMessage.
///
/// Supported event shapes:
/// - Legacy `omp` schema: `message`, `tool_use`, `tool_result`, `done`, `error`
/// - Modern Pi runner schema (pi >= 0.45.1):
///   - `message_update` with `assistantMessageEvent.type=text_delta`
///   - `message_end` (final assistant message)
///   - `tool_execution_start` / `tool_execution_end`
///   - `turn_end` / `agent_end` (final assistant message repeated)
fn parse_stream_message(json: &Value) -> Option<StreamMessage> {
    let msg_type = json.get("type")?.as_str()?;

    match msg_type {
        // ------------------------------------------------------------------
        // Modern Pi runner schema
        // ------------------------------------------------------------------
        "message_update" => {
            let ev = json.get("assistantMessageEvent")?;
            let ev_type = ev.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if ev_type != "text_delta" {
                return None;
            }
            let delta = ev.get("delta").and_then(|v| v.as_str()).unwrap_or("");
            if delta.is_empty() {
                return None;
            }
            debug_log(&format!("message_update delta: {} chars", delta.len()));
            Some(StreamMessage::Text {
                content: delta.to_string(),
            })
        }

        "tool_execution_start" => {
            let tool_name_raw = json.get("toolName")
                .or_else(|| json.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let tool_name = normalize_tool_name(tool_name_raw);
            let args = json.get("args").unwrap_or(&Value::Null);
            let input = format_pi_tool_args_for_display(&tool_name, args);
            debug_log(&format!("tool_execution_start: {}", tool_name));
            Some(StreamMessage::ToolUse { name: tool_name, input })
        }

        "tool_execution_end" => {
            let is_error = json.get("isError")
                .or_else(|| json.get("is_error"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let result = json.get("result").unwrap_or(&Value::Null);
            let content_val = result.get("content").unwrap_or(result);
            let content = extract_text_from_content_value(content_val);
            debug_log(&format!("tool_execution_end: {} chars, error={}", content.len(), is_error));
            Some(StreamMessage::ToolResult { content, is_error })
        }

        "message_end" => {
            let Some(message) = json.get("message") else {
                return None;
            };
            let role = message.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role != "assistant" {
                return None;
            }

            // Pi uses stopReason=toolUse for intermediate assistant tool-call messages.
            let stop_reason = message.get("stopReason").and_then(|v| v.as_str()).unwrap_or("");
            if stop_reason.to_ascii_lowercase() == "tooluse" {
                return None;
            }

            let result = message
                .get("content")
                .map(extract_text_from_content_value)
                .unwrap_or_default();
            if result.trim().is_empty() {
                return None;
            }

            debug_log(&format!("message_end (assistant): {} chars", result.len()));
            Some(StreamMessage::Done {
                result,
                session_id: None,
            })
        }

        "agent_end" => {
            // Prefer the last assistant message in the array (if present).
            let mut result = String::new();
            if let Some(messages) = json.get("messages").and_then(|v| v.as_array()) {
                for m in messages {
                    if let Some(text) = extract_assistant_text_from_message(m) {
                        result = text;
                    }
                }
            }
            debug_log(&format!("agent_end: {} chars", result.len()));
            Some(StreamMessage::Done {
                result,
                session_id: None,
            })
        }

        // ------------------------------------------------------------------
        // Legacy `omp` schema
        // ------------------------------------------------------------------
        "message" => {
            let role = json.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role != "assistant" {
                return None;
            }
            let content = json.get("content")
                .map(extract_text_from_content_value)
                .unwrap_or_default();
            if content.is_empty() {
                return None;
            }
            debug_log(&format!("message: {} chars", content.len()));
            Some(StreamMessage::Text { content })
        }

        "tool_use" => {
            let tool_name_raw = json.get("name")
                .or_else(|| json.get("toolName"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let tool_name = normalize_tool_name(tool_name_raw);
            let input = json.get("input")
                .or_else(|| json.get("arguments"))
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    _ => serde_json::to_string_pretty(v).unwrap_or_default(),
                })
                .unwrap_or_default();
            debug_log(&format!("tool_use: {}", tool_name));
            Some(StreamMessage::ToolUse { name: tool_name, input })
        }

        "tool_result" => {
            let is_error = json.get("isError")
                .or_else(|| json.get("is_error"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let content = json.get("result")
                .or_else(|| json.get("content"))
                .or_else(|| json.get("output"))
                .map(extract_text_from_content_value)
                .unwrap_or_default();
            debug_log(&format!("tool_result: {} chars, error={}", content.len(), is_error));
            Some(StreamMessage::ToolResult { content, is_error })
        }

        "done" | "complete" | "end" => {
            let session_id = json.get("sessionId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            debug_log("done");
            Some(StreamMessage::Done {
                result: String::new(),
                session_id,
            })
        }

        "error" => {
            let message = json.get("error")
                .or_else(|| json.get("message"))
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    Value::Object(obj) => obj.get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown error")
                        .to_string(),
                    _ => serde_json::to_string(v).unwrap_or_else(|_| "Unknown error".to_string()),
                })
                .unwrap_or_else(|| "Unknown error".to_string());
            debug_log(&format!("error: {}", message));
            Some(StreamMessage::Error { message })
        }

        // compaction, modelChange, thinkingLevelChange, injection — skip
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_omp_jsonl_output_success() {
        let output = r#"{"type":"message","role":"user","content":"hello","sessionId":"sess-123","timestamp":"2025-01-01T00:00:00Z"}
{"type":"message","role":"assistant","content":"Hello, world!","sessionId":"sess-123","timestamp":"2025-01-01T00:00:01Z"}
{"type":"done","sessionId":"sess-123"}"#;
        let response = parse_omp_jsonl_output(output);
        assert!(response.success);
        assert_eq!(response.response, Some("Hello, world!".to_string()));
        assert_eq!(response.session_id, Some("sess-123".to_string()));
    }

    #[test]
    fn test_parse_omp_jsonl_output_empty() {
        let output = r#"{"type":"message","role":"user","content":"hello","sessionId":"sess-123"}
{"type":"done","sessionId":"sess-123"}"#;
        let response = parse_omp_jsonl_output(output);
        assert!(!response.success);
        assert!(response.response.is_none());
        assert_eq!(response.session_id, Some("sess-123".to_string()));
    }

    #[test]
    fn test_parse_omp_jsonl_output_pi_schema_message_end() {
        // Based on Pi runner JSONL schema (pi >= 0.45.1)
        let output = r#"{"type":"session","id":"sess-pi-1","version":3,"timestamp":"2026-01-13T00:33:34.702Z","cwd":"/repo"}
{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"Done."}],"stopReason":"stop"}}"#;
        let response = parse_omp_jsonl_output(output);
        assert!(response.success);
        assert_eq!(response.response, Some("Done.".to_string()));
        assert_eq!(response.session_id, Some("sess-pi-1".to_string()));
    }

    #[test]
    fn test_parse_stream_message_update_text_delta() {
        let json: Value = serde_json::from_str(
            r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Hi","contentIndex":0}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::Text { content }) => assert_eq!(content, "Hi"),
            _ => panic!("Expected Text delta"),
        }
    }

    #[test]
    fn test_parse_stream_tool_execution_start() {
        let json: Value = serde_json::from_str(
            r#"{"type":"tool_execution_start","toolCallId":"tool_1","toolName":"bash","args":{"command":"ls"}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::ToolUse { name, input }) => {
                assert_eq!(name, "Bash");
                assert!(input.contains("ls"));
            }
            _ => panic!("Expected ToolUse"),
        }
    }

    #[test]
    fn test_parse_stream_tool_execution_end() {
        let json: Value = serde_json::from_str(
            r#"{"type":"tool_execution_end","toolCallId":"tool_1","toolName":"bash","result":{"content":[{"type":"text","text":"ok"}],"details":{}},"isError":false}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::ToolResult { content, is_error }) => {
                assert_eq!(content, "ok");
                assert!(!is_error);
            }
            _ => panic!("Expected ToolResult"),
        }
    }

    #[test]
    fn test_parse_stream_message_end_done() {
        let json: Value = serde_json::from_str(
            r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"Done."}],"stopReason":"stop"}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::Done { result, .. }) => assert_eq!(result, "Done."),
            _ => panic!("Expected Done"),
        }
    }
    #[test]
    fn test_parse_stream_message_end_tool_use_not_done() {
        let json: Value = serde_json::from_str(
            r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"toolCall","id":"t1","name":"read","arguments":{}}],"stopReason":"toolUse"}}"#
        ).unwrap();
        assert!(parse_stream_message(&json).is_none());
    }
    #[test]
    fn test_parse_stream_message_assistant() {
        let json: Value = serde_json::from_str(
            r#"{"type":"message","role":"assistant","content":"Hello from oh-my-pi","sessionId":"sess-123"}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::Text { content }) => assert_eq!(content, "Hello from oh-my-pi"),
            _ => panic!("Expected Text message"),
        }
    }

    #[test]
    fn test_parse_stream_message_user_skipped() {
        let json: Value = serde_json::from_str(
            r#"{"type":"message","role":"user","content":"hello","sessionId":"sess-123"}"#
        ).unwrap();
        assert!(parse_stream_message(&json).is_none());
    }

    #[test]
    fn test_parse_stream_message_empty_content_skipped() {
        let json: Value = serde_json::from_str(
            r#"{"type":"message","role":"assistant","content":"","sessionId":"sess-123"}"#
        ).unwrap();
        assert!(parse_stream_message(&json).is_none());
    }

    #[test]
    fn test_parse_stream_tool_use() {
        let json: Value = serde_json::from_str(
            r#"{"type":"tool_use","name":"bash","input":{"command":"ls -la"},"sessionId":"sess-123"}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::ToolUse { name, input }) => {
                assert_eq!(name, "Bash");
                assert!(input.contains("ls -la"));
            }
            _ => panic!("Expected ToolUse message"),
        }
    }

    #[test]
    fn test_parse_stream_tool_use_with_tool_name_field() {
        let json: Value = serde_json::from_str(
            r#"{"type":"tool_use","toolName":"edit","arguments":{"file":"test.rs"}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::ToolUse { name, input }) => {
                assert_eq!(name, "Edit");
                assert!(input.contains("test.rs"));
            }
            _ => panic!("Expected ToolUse message"),
        }
    }

    #[test]
    fn test_parse_stream_tool_result_success() {
        let json: Value = serde_json::from_str(
            r#"{"type":"tool_result","toolName":"bash","result":"file.txt\ndir/","isError":false}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::ToolResult { content, is_error }) => {
                assert_eq!(content, "file.txt\ndir/");
                assert!(!is_error);
            }
            _ => panic!("Expected ToolResult message"),
        }
    }

    #[test]
    fn test_parse_stream_tool_result_error() {
        let json: Value = serde_json::from_str(
            r#"{"type":"tool_result","toolName":"bash","result":"command not found","isError":true}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::ToolResult { content, is_error }) => {
                assert_eq!(content, "command not found");
                assert!(is_error);
            }
            _ => panic!("Expected ToolResult message with error"),
        }
    }

    #[test]
    fn test_parse_stream_done() {
        let json: Value = serde_json::from_str(
            r#"{"type":"done","sessionId":"sess-123"}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::Done { result, session_id }) => {
                assert!(result.is_empty());
                assert_eq!(session_id, Some("sess-123".to_string()));
            }
            _ => panic!("Expected Done message"),
        }
    }

    #[test]
    fn test_parse_stream_error_string() {
        let json: Value = serde_json::from_str(
            r#"{"type":"error","error":"Connection lost","sessionId":"sess-123"}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::Error { message }) => {
                assert_eq!(message, "Connection lost");
            }
            _ => panic!("Expected Error message"),
        }
    }

    #[test]
    fn test_parse_stream_error_object() {
        let json: Value = serde_json::from_str(
            r#"{"type":"error","error":{"message":"Rate limit exceeded","code":429}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::Error { message }) => {
                assert_eq!(message, "Rate limit exceeded");
            }
            _ => panic!("Expected Error message"),
        }
    }

    #[test]
    fn test_parse_stream_compaction_skipped() {
        let json: Value = serde_json::from_str(
            r#"{"type":"compaction","summary":"Context compacted","originalLength":5000}"#
        ).unwrap();
        assert!(parse_stream_message(&json).is_none());
    }

    #[test]
    fn test_parse_stream_model_change_skipped() {
        let json: Value = serde_json::from_str(
            r#"{"type":"modelChange","from":"gpt-4","to":"claude-3"}"#
        ).unwrap();
        assert!(parse_stream_message(&json).is_none());
    }

    #[test]
    fn test_parse_stream_unknown_type() {
        let json: Value = serde_json::from_str(
            r#"{"type":"unknown_event","data":"something"}"#
        ).unwrap();
        assert!(parse_stream_message(&json).is_none());
    }

    // --- is_session_not_found_error ---

    #[test]
    fn test_session_not_found_omp_error() {
        assert!(is_session_not_found_error(
            r#"Error: [Uncaught Exception] Error: Session "oh-my-pi-1772278729103" not found."#
        ));
    }

    #[test]
    fn test_session_not_found_simple() {
        assert!(is_session_not_found_error("Session not found"));
    }

    #[test]
    fn test_session_not_found_case_insensitive() {
        assert!(is_session_not_found_error("SESSION NOT FOUND"));
    }

    #[test]
    fn test_session_not_found_negative() {
        assert!(!is_session_not_found_error("Connection lost"));
        assert!(!is_session_not_found_error("Rate limit exceeded"));
        assert!(!is_session_not_found_error("File not found"));
    }
}
