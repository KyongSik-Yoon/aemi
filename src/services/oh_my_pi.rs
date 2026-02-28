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
                AgentResponse {
                    success: false,
                    response: None,
                    session_id: None,
                    error: Some(if stderr.is_empty() {
                        format!("Process exited with code {:?}", output.status.code())
                    } else {
                        stderr
                    }),
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

/// Parse oh-my-pi CLI JSONL output: extract sessionId and last text content
#[allow(dead_code)]
fn parse_omp_jsonl_output(output: &str) -> AgentResponse {
    let mut session_id: Option<String> = None;
    let mut last_text: Option<String> = None;

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Ok(json) = serde_json::from_str::<Value>(line) {
            // Capture sessionId from any event
            if session_id.is_none() {
                session_id = json.get("sessionId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }

            let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if msg_type == "message" {
                let role = json.get("role").and_then(|v| v.as_str()).unwrap_or("");
                if role == "assistant" {
                    if let Some(text) = json.get("content").and_then(|v| v.as_str()) {
                        if !text.is_empty() {
                            last_text = Some(text.to_string());
                        }
                    }
                }
            }
        }
    }

    AgentResponse {
        success: last_text.is_some(),
        response: last_text,
        session_id,
        error: None,
    }
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
    args.push(effective_prompt);

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

    // oh-my-pi uses a custom handler because sessionId must be extracted from the
    // JSON envelope (not from parse_stream_message) and Init must be sent manually.
    provider_common::run_streaming(
        &config,
        sender,
        cancel_token,
        |json, sender, state| {
            // Capture sessionId from any event
            if state.session_id.is_none() {
                state.session_id = json.get("sessionId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }

            // Send Init on first event with sessionId
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
        },
    )
}

/// Parse an oh-my-pi JSONL event into a StreamMessage.
///
/// oh-my-pi JSONL event types (emitted by `omp --print --mode json`):
/// - message:        text message from user or assistant
/// - tool_use:       tool invocation (with name and input)
/// - tool_result:    tool execution result
/// - compaction:     context compaction summary (skipped)
/// - error:          session-level error
/// - done:           session completion
fn parse_stream_message(json: &Value) -> Option<StreamMessage> {
    let msg_type = json.get("type")?.as_str()?;

    match msg_type {
        "message" => {
            let role = json.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role != "assistant" { return None; }

            let content = json.get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if content.is_empty() { return None; }
            debug_log(&format!("message: {} chars", content.len()));
            Some(StreamMessage::Text { content })
        }

        "tool_use" => {
            let tool_name = json.get("name")
                .or_else(|| json.get("toolName"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let input = json.get("input")
                .or_else(|| json.get("arguments"))
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    _ => serde_json::to_string_pretty(v).unwrap_or_default(),
                })
                .unwrap_or_default();
            debug_log(&format!("tool_use: {}", tool_name));
            Some(StreamMessage::ToolUse {
                name: tool_name,
                input,
            })
        }

        "tool_result" => {
            let is_error = json.get("isError")
                .or_else(|| json.get("is_error"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let content = json.get("result")
                .or_else(|| json.get("content"))
                .or_else(|| json.get("output"))
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    _ => serde_json::to_string_pretty(v).unwrap_or_default(),
                })
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
                    Value::Object(obj) => {
                        obj.get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("Unknown error")
                            .to_string()
                    }
                    _ => serde_json::to_string(v).unwrap_or_else(|_| "Unknown error".to_string()),
                })
                .unwrap_or_else(|| "Unknown error".to_string());
            debug_log(&format!("error: {}", message));
            Some(StreamMessage::Error { message })
        }

        // compaction, modelChange, thinkingLevelChange, injection — skip
        _ => None
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
                assert_eq!(name, "bash");
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
                assert_eq!(name, "edit");
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
}
