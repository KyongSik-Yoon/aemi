use std::process::Stdio;
use std::sync::mpsc::Sender;
use serde_json::Value;

pub use super::agent::{StreamMessage, CancelToken, AgentResponse};
use super::provider_common::{self, StreamingConfig};

// Generate resolve_binary_path(), get_binary_path(), is_cli_available(), debug_log() for "gemini"
define_ai_service_helpers!("gemini");

/// Check if Gemini CLI is available
#[allow(dead_code)]
pub fn is_gemini_available() -> bool {
    is_cli_available()
}

/// Execute a command using Gemini CLI (non-streaming)
#[allow(dead_code)]
pub fn execute_command(
    prompt: &str,
    working_dir: &str,
) -> AgentResponse {
    // Gemini CLI: -p/--prompt takes the prompt text as its argument value (not stdin)
    let args = vec![
        "-p".to_string(),
        prompt.to_string(),
        "--output-format".to_string(),
        "json".to_string(),
        "--yolo".to_string(),
    ];

    let gemini_bin = match get_binary_path() {
        Some(path) => path,
        None => {
            return AgentResponse {
                success: false,
                response: None,
                session_id: None,
                error: Some("Gemini CLI not found. Is Gemini CLI installed?".to_string()),
            };
        }
    };

    let child = match Command::new(gemini_bin)
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
                error: Some(format!("Failed to start Gemini: {}. Is Gemini CLI installed?", e)),
            };
        }
    };

    // Wait for output
    match child.wait_with_output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                parse_gemini_json_output(&stdout)
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

/// Parse Gemini CLI JSON output: { "response": "...", "stats": {...}, "error": null }
#[allow(dead_code)]
fn parse_gemini_json_output(output: &str) -> AgentResponse {
    if let Ok(json) = serde_json::from_str::<Value>(output.trim()) {
        // Check for error field
        if let Some(err) = json.get("error") {
            if !err.is_null() {
                let message = err.get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error")
                    .to_string();
                return AgentResponse {
                    success: false,
                    response: None,
                    session_id: None,
                    error: Some(message),
                };
            }
        }

        let response = json.get("response")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        AgentResponse {
            success: response.is_some(),
            response,
            session_id: None, // Gemini non-interactive has no session continuity
            error: None,
        }
    } else {
        // Fallback: treat as plain text
        AgentResponse {
            success: true,
            response: Some(output.trim().to_string()),
            session_id: None,
            error: None,
        }
    }
}

/// Execute a command using Gemini CLI with streaming output
pub fn execute_command_streaming(
    prompt: &str,
    _session_id: Option<&str>, // Gemini non-interactive is single-turn, session_id ignored
    working_dir: &str,
    sender: Sender<StreamMessage>,
    system_prompt: Option<&str>,
    _allowed_tools: Option<&[String]>, // Gemini uses --yolo instead of tool allowlist
    cancel_token: Option<std::sync::Arc<CancelToken>>,
) -> Result<(), String> {
    debug_log(&format!("prompt_len: {} chars", prompt.len()));

    let effective_prompt = provider_common::build_effective_prompt(system_prompt, prompt);

    // Gemini CLI: -p/--prompt takes the prompt text as its argument value (not stdin)
    let args = vec![
        "-p".to_string(),
        effective_prompt,
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--yolo".to_string(),
    ];

    let binary_path = get_binary_path()
        .ok_or_else(|| {
            debug_log("ERROR: Gemini CLI not found");
            "Gemini CLI not found. Is Gemini CLI installed?".to_string()
        })?;

    let config = StreamingConfig {
        provider_name: "gemini",
        binary_path,
        args: &args,
        working_dir,
        env_vars: &[],
        env_remove: &[],
        stdin_data: None,
        send_synthetic_init: true,
    };

    provider_common::run_streaming(
        &config,
        sender,
        cancel_token,
        provider_common::make_default_handler(parse_stream_message),
    )
}

/// Parse a Gemini stream-json line into a StreamMessage.
///
/// Gemini CLI (TerminaI) stream-json events:
/// - init: {"type":"init","session_id":"...","model":"..."}
/// - message: {"type":"message","role":"assistant","content":"...","delta":true}
/// - tool_use: {"type":"tool_use","tool_name":"...","tool_id":"...","parameters":{...}}
/// - tool_result: {"type":"tool_result","tool_id":"...","status":"success|error","output":"..."}
/// - result: {"type":"result","status":"success|error","error":{...},"stats":{...}}
fn parse_stream_message(json: &Value) -> Option<StreamMessage> {
    let msg_type = json.get("type")?.as_str()?;

    match msg_type {
        "init" => {
            // {"type":"init","timestamp":"...","session_id":"...","model":"..."}
            let session_id = json.get("session_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            debug_log(&format!("Gemini init: session_id={}", session_id));
            Some(StreamMessage::Init { session_id })
        }
        "message" => {
            // {"type":"message","role":"assistant","content":"text here","delta":true}
            let role = json.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role != "assistant" {
                return None;
            }

            // Gemini CLI sends content as a plain string (not an array)
            if let Some(text) = json.get("content").and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    return Some(StreamMessage::Text { content: text.to_string() });
                }
            }
            None
        }
        "tool_use" => {
            // {"type":"tool_use","tool_name":"run_terminal_command","tool_id":"...","parameters":{...}}
            let name = json.get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let input = json.get("parameters")
                .map(|v| serde_json::to_string_pretty(v).unwrap_or_default())
                .unwrap_or_default();
            Some(StreamMessage::ToolUse { name, input })
        }
        "tool_result" => {
            // {"type":"tool_result","tool_id":"...","status":"success","output":"..."}
            let status = json.get("status").and_then(|v| v.as_str()).unwrap_or("");
            let is_error = status != "success";
            let content = json.get("output")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Some(StreamMessage::ToolResult { content, is_error })
        }
        "result" => {
            // {"type":"result","status":"success|error","error":{...},"stats":{...}}
            let status = json.get("status").and_then(|v| v.as_str()).unwrap_or("");
            if status == "error" {
                let error_msg = json.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error")
                    .to_string();
                Some(StreamMessage::Error { message: error_msg })
            } else {
                Some(StreamMessage::Done { result: String::new(), session_id: None })
            }
        }
        _ => None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gemini_json_output_success() {
        let output = r#"{"response": "Hello, world!", "stats": {}, "error": null}"#;
        let response = parse_gemini_json_output(output);
        assert!(response.success);
        assert_eq!(response.response, Some("Hello, world!".to_string()));
        assert!(response.session_id.is_none());
    }

    #[test]
    fn test_parse_gemini_json_output_error() {
        let output = r#"{"response": null, "stats": {}, "error": {"type": "AuthError", "message": "Authentication failed"}}"#;
        let response = parse_gemini_json_output(output);
        assert!(!response.success);
        assert_eq!(response.error, Some("Authentication failed".to_string()));
    }

    #[test]
    fn test_parse_gemini_json_output_plain_text() {
        let output = "Just plain text";
        let response = parse_gemini_json_output(output);
        assert!(response.success);
        assert_eq!(response.response, Some("Just plain text".to_string()));
    }

    #[test]
    fn test_parse_stream_init() {
        let json: Value = serde_json::from_str(
            r#"{"type":"init","timestamp":"2026-02-24T09:21:14.289Z","session_id":"c1fd9e90-3060","model":"auto-gemini-3"}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::Init { session_id }) => assert_eq!(session_id, "c1fd9e90-3060"),
            _ => panic!("Expected Init message"),
        }
    }

    #[test]
    fn test_parse_stream_message_text() {
        let json: Value = serde_json::from_str(
            r#"{"type":"message","timestamp":"2026-02-24T09:21:25.050Z","role":"assistant","content":"Hello world","delta":true}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::Text { content }) => assert_eq!(content, "Hello world"),
            _ => panic!("Expected Text message"),
        }
    }

    #[test]
    fn test_parse_stream_message_tool_use() {
        let json: Value = serde_json::from_str(
            r#"{"type":"tool_use","timestamp":"2026-02-24T09:21:31.169Z","tool_name":"run_terminal_command","tool_id":"run_terminal_command-123","parameters":{"command":"ls -la","description":"List files"}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::ToolUse { name, input }) => {
                assert_eq!(name, "run_terminal_command");
                assert!(input.contains("ls -la"));
            }
            _ => panic!("Expected ToolUse message"),
        }
    }

    #[test]
    fn test_parse_stream_message_tool_result_success() {
        let json: Value = serde_json::from_str(
            r#"{"type":"tool_result","timestamp":"2026-02-24T09:21:26.415Z","tool_id":"tool-123","status":"success","output":"file.txt\ndir/"}"#
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
    fn test_parse_stream_message_tool_result_error() {
        let json: Value = serde_json::from_str(
            r#"{"type":"tool_result","tool_id":"tool-456","status":"error","output":"command not found"}"#
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
    fn test_parse_stream_message_result_success() {
        let json: Value = serde_json::from_str(
            r#"{"type":"result","timestamp":"2026-02-24T09:22:00.000Z","status":"success","stats":{"total_tokens":100}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::Done { result, session_id }) => {
                assert!(result.is_empty());
                assert!(session_id.is_none());
            }
            _ => panic!("Expected Done message"),
        }
    }

    #[test]
    fn test_parse_stream_message_result_error() {
        let json: Value = serde_json::from_str(
            r#"{"type":"result","status":"error","error":{"type":"Error","message":"[API Error: quota exceeded]"},"stats":{"total_tokens":100}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::Error { message }) => {
                assert_eq!(message, "[API Error: quota exceeded]");
            }
            _ => panic!("Expected Error message"),
        }
    }

    #[test]
    fn test_parse_stream_message_user_message_ignored() {
        let json: Value = serde_json::from_str(
            r#"{"type":"message","role":"user","content":"hello"}"#
        ).unwrap();
        assert!(parse_stream_message(&json).is_none());
    }

    #[test]
    fn test_parse_stream_message_unknown_type() {
        let json: Value = serde_json::from_str(
            r#"{"type":"unknown","data":"something"}"#
        ).unwrap();
        assert!(parse_stream_message(&json).is_none());
    }
}
