use std::process::Stdio;
use std::sync::mpsc::Sender;
use serde_json::Value;

pub use super::agent::{StreamMessage, CancelToken, AgentResponse};
use super::provider_common::{self, StreamingConfig};

// Generate resolve_binary_path(), get_binary_path(), is_cli_available(), debug_log() for "opencode"
define_ai_service_helpers!("opencode");

/// Check if OpenCode CLI is available
#[allow(dead_code)]
pub fn is_opencode_available() -> bool {
    is_cli_available()
}

/// Execute a command using OpenCode CLI (non-streaming)
#[allow(dead_code)]
pub fn execute_command(
    prompt: &str,
    session_id: Option<&str>,
    working_dir: &str,
) -> AgentResponse {
    let mut args = vec![
        "run".to_string(),
        "--format".to_string(),
        "json".to_string(),
    ];

    // Session resume support
    if let Some(sid) = session_id {
        args.push("--session".to_string());
        args.push(sid.to_string());
    }

    // Prompt as positional argument (last)
    args.push(prompt.to_string());

    let opencode_bin = match get_binary_path() {
        Some(path) => path,
        None => {
            return AgentResponse {
                success: false,
                response: None,
                session_id: None,
                error: Some("OpenCode CLI not found. Is OpenCode CLI installed?".to_string()),
            };
        }
    };

    let child = match Command::new(opencode_bin)
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
                error: Some(format!("Failed to start OpenCode: {}. Is OpenCode CLI installed?", e)),
            };
        }
    };

    // Wait for output — parse JSONL and extract last text
    match child.wait_with_output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                parse_opencode_jsonl_output(&stdout)
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

/// Parse OpenCode CLI JSONL output: extract sessionID and last text content
#[allow(dead_code)]
fn parse_opencode_jsonl_output(output: &str) -> AgentResponse {
    let mut session_id: Option<String> = None;
    let mut last_text: Option<String> = None;

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Ok(json) = serde_json::from_str::<Value>(line) {
            // Capture sessionID from any event
            if session_id.is_none() {
                session_id = json.get("sessionID")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }

            let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if msg_type == "text" {
                if let Some(part) = json.get("part") {
                    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
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

/// Execute a command using OpenCode CLI with streaming output
pub fn execute_command_streaming(
    prompt: &str,
    session_id: Option<&str>,
    working_dir: &str,
    sender: Sender<StreamMessage>,
    system_prompt: Option<&str>,
    _allowed_tools: Option<&[String]>, // OpenCode manages tools internally
    cancel_token: Option<std::sync::Arc<CancelToken>>,
) -> Result<(), String> {
    debug_log(&format!("prompt_len: {} chars", prompt.len()));
    debug_log(&format!("session_id: {:?}", session_id));

    let effective_prompt = provider_common::build_effective_prompt(system_prompt, prompt);

    // Build args: opencode run --format json [--session <session_id>] "prompt"
    let mut args = vec![
        "run".to_string(),
        "--format".to_string(),
        "json".to_string(),
    ];

    // Session resume support
    if let Some(sid) = session_id {
        debug_log(&format!("Resuming session: {}", sid));
        args.push("--session".to_string());
        args.push(sid.to_string());
    }

    // Prompt as positional argument (must be last)
    args.push(effective_prompt);

    let binary_path = get_binary_path()
        .ok_or_else(|| {
            debug_log("ERROR: OpenCode CLI not found");
            "OpenCode CLI not found. Is OpenCode CLI installed?".to_string()
        })?;

    let config = StreamingConfig {
        provider_name: "opencode",
        binary_path,
        args: &args,
        working_dir,
        env_vars: &[],
        env_remove: &[],
        stdin_data: None,
        send_synthetic_init: true,
    };

    // OpenCode uses a custom handler because sessionID must be extracted from the
    // JSON envelope (not from parse_stream_message) and Init must be sent manually.
    provider_common::run_streaming(
        &config,
        sender,
        cancel_token,
        |json, sender, state| {
            // Capture sessionID from any event
            if state.session_id.is_none() {
                state.session_id = json.get("sessionID")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }

            // Send Init on first event with sessionID
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

/// Parse an OpenCode JSONL event into a StreamMessage.
///
/// OpenCode JSONL event types (emitted by `opencode run --format json`):
/// - step_start:  beginning of a processing step
/// - step_finish: completion of a processing step
/// - text:        text response from the assistant
/// - tool_use:    tool execution (with state: running/completed/error)
/// - reasoning:   thinking/chain-of-thought (skipped)
/// - error:       session-level error
fn parse_stream_message(json: &Value) -> Option<StreamMessage> {
    let msg_type = json.get("type")?.as_str()?;

    match msg_type {
        "text" => {
            // {"type":"text","timestamp":...,"sessionID":"...","part":{"type":"text","text":"...","time":{...}}}
            let part = json.get("part")?;
            let text = part.get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if text.is_empty() { return None; }
            debug_log(&format!("text: {} chars", text.len()));
            Some(StreamMessage::Text { content: text })
        }

        "tool_use" => {
            // {"type":"tool_use","timestamp":...,"sessionID":"...","part":{"type":"tool","tool":"bash","state":{"status":"running","input":{...}}}}
            let part = json.get("part")?;
            let tool_name = part.get("tool")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let state = part.get("state")?;
            let status = state.get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            match status {
                "running" | "pending" => {
                    // Tool execution started
                    let input = state.get("input")
                        .map(|v| serde_json::to_string_pretty(v).unwrap_or_default())
                        .unwrap_or_default();
                    debug_log(&format!("tool_use start: {} ({})", tool_name, status));
                    Some(StreamMessage::ToolUse {
                        name: tool_name,
                        input,
                    })
                }
                "completed" => {
                    // Tool execution completed
                    let output = state.get("output")
                        .map(|v| match v {
                            Value::String(s) => s.clone(),
                            _ => serde_json::to_string_pretty(v).unwrap_or_default(),
                        })
                        .unwrap_or_default();
                    debug_log(&format!("tool_use completed: {}", tool_name));
                    Some(StreamMessage::ToolResult {
                        content: output,
                        is_error: false,
                    })
                }
                "error" => {
                    // Tool execution failed
                    let error_msg = state.get("error")
                        .map(|v| match v {
                            Value::String(s) => s.clone(),
                            _ => serde_json::to_string_pretty(v).unwrap_or_default(),
                        })
                        .unwrap_or_else(|| "Tool execution failed".to_string());
                    debug_log(&format!("tool_use error: {}: {}", tool_name, error_msg));
                    Some(StreamMessage::ToolResult {
                        content: error_msg,
                        is_error: true,
                    })
                }
                _ => None
            }
        }

        "step_finish" => {
            // {"type":"step_finish","timestamp":...,"sessionID":"...","part":{...}}
            debug_log("step_finish");
            let session_id = json.get("sessionID")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Some(StreamMessage::Done {
                result: String::new(),
                session_id,
            })
        }

        "error" => {
            // {"type":"error","timestamp":...,"sessionID":"...","error":{...}}
            let message = json.get("error")
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

        // step_start, reasoning — skip
        _ => None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_opencode_jsonl_output_success() {
        let output = r#"{"type":"step_start","timestamp":1700000000000,"sessionID":"sess-123","part":{}}
{"type":"text","timestamp":1700000001000,"sessionID":"sess-123","part":{"type":"text","text":"Hello, world!","time":{"end":1700000001000}}}
{"type":"step_finish","timestamp":1700000002000,"sessionID":"sess-123","part":{}}"#;
        let response = parse_opencode_jsonl_output(output);
        assert!(response.success);
        assert_eq!(response.response, Some("Hello, world!".to_string()));
        assert_eq!(response.session_id, Some("sess-123".to_string()));
    }

    #[test]
    fn test_parse_opencode_jsonl_output_empty() {
        let output = r#"{"type":"step_start","timestamp":1700000000000,"sessionID":"sess-123","part":{}}
{"type":"step_finish","timestamp":1700000001000,"sessionID":"sess-123","part":{}}"#;
        let response = parse_opencode_jsonl_output(output);
        assert!(!response.success);
        assert!(response.response.is_none());
        assert_eq!(response.session_id, Some("sess-123".to_string()));
    }

    #[test]
    fn test_parse_stream_text() {
        let json: Value = serde_json::from_str(
            r#"{"type":"text","timestamp":1700000000000,"sessionID":"sess-123","part":{"type":"text","text":"Hello from OpenCode","time":{"end":1700000000000}}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::Text { content }) => assert_eq!(content, "Hello from OpenCode"),
            _ => panic!("Expected Text message"),
        }
    }

    #[test]
    fn test_parse_stream_text_empty_skipped() {
        let json: Value = serde_json::from_str(
            r#"{"type":"text","timestamp":1700000000000,"sessionID":"sess-123","part":{"type":"text","text":"","time":{}}}"#
        ).unwrap();
        assert!(parse_stream_message(&json).is_none());
    }

    #[test]
    fn test_parse_stream_tool_use_running() {
        let json: Value = serde_json::from_str(
            r#"{"type":"tool_use","timestamp":1700000000000,"sessionID":"sess-123","part":{"type":"tool","tool":"bash","state":{"status":"running","input":{"command":"ls -la"}}}}"#
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
    fn test_parse_stream_tool_use_pending() {
        let json: Value = serde_json::from_str(
            r#"{"type":"tool_use","timestamp":1700000000000,"sessionID":"sess-123","part":{"type":"tool","tool":"glob","state":{"status":"pending","input":{"pattern":"*.rs"}}}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::ToolUse { name, input }) => {
                assert_eq!(name, "glob");
                assert!(input.contains("*.rs"));
            }
            _ => panic!("Expected ToolUse message"),
        }
    }

    #[test]
    fn test_parse_stream_tool_use_completed() {
        let json: Value = serde_json::from_str(
            r#"{"type":"tool_use","timestamp":1700000000000,"sessionID":"sess-123","part":{"type":"tool","tool":"bash","state":{"status":"completed","input":{"command":"ls"},"output":"file.txt\ndir/"}}}"#
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
    fn test_parse_stream_tool_use_error() {
        let json: Value = serde_json::from_str(
            r#"{"type":"tool_use","timestamp":1700000000000,"sessionID":"sess-123","part":{"type":"tool","tool":"bash","state":{"status":"error","error":"Command not found"}}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::ToolResult { content, is_error }) => {
                assert_eq!(content, "Command not found");
                assert!(is_error);
            }
            _ => panic!("Expected ToolResult message with error"),
        }
    }

    #[test]
    fn test_parse_stream_step_finish() {
        let json: Value = serde_json::from_str(
            r#"{"type":"step_finish","timestamp":1700000000000,"sessionID":"sess-123","part":{}}"#
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
            r#"{"type":"error","timestamp":1700000000000,"sessionID":"sess-123","error":"Connection lost"}"#
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
            r#"{"type":"error","timestamp":1700000000000,"sessionID":"sess-123","error":{"message":"Rate limit exceeded","code":429}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::Error { message }) => {
                assert_eq!(message, "Rate limit exceeded");
            }
            _ => panic!("Expected Error message"),
        }
    }

    #[test]
    fn test_parse_stream_step_start_skipped() {
        let json: Value = serde_json::from_str(
            r#"{"type":"step_start","timestamp":1700000000000,"sessionID":"sess-123","part":{}}"#
        ).unwrap();
        assert!(parse_stream_message(&json).is_none());
    }

    #[test]
    fn test_parse_stream_reasoning_skipped() {
        let json: Value = serde_json::from_str(
            r#"{"type":"reasoning","timestamp":1700000000000,"sessionID":"sess-123","part":{"type":"reasoning","text":"Let me think..."}}"#
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

    #[test]
    fn test_parse_stream_tool_use_object_output() {
        let json: Value = serde_json::from_str(
            r#"{"type":"tool_use","timestamp":1700000000000,"sessionID":"sess-123","part":{"type":"tool","tool":"read","state":{"status":"completed","input":{"path":"test.rs"},"output":{"content":"fn main() {}"}}}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::ToolResult { content, is_error }) => {
                assert!(content.contains("fn main()"));
                assert!(!is_error);
            }
            _ => panic!("Expected ToolResult message"),
        }
    }
}
