use std::process::Stdio;
use std::sync::mpsc::Sender;
use regex::Regex;
use serde_json::Value;

pub use super::agent::{StreamMessage, CancelToken, AgentResponse};
use super::provider_common::{self, StreamingConfig, DEFAULT_SYSTEM_PROMPT};

// Generate resolve_binary_path(), get_binary_path(), is_cli_available(), debug_log() for "claude"
define_ai_service_helpers!("claude");

/// Type alias for backward compatibility
pub type ClaudeResponse = AgentResponse;

/// Cached regex pattern for session ID validation
fn session_id_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"^[a-zA-Z0-9_-]+$").expect("Invalid session ID regex pattern"))
}

/// Validate session ID format (alphanumeric, dashes, underscores only)
/// Max length reduced to 64 characters for security
fn is_valid_session_id(session_id: &str) -> bool {
    !session_id.is_empty() && session_id.len() <= 64 && session_id_regex().is_match(session_id)
}

/// Default allowed tools for Claude CLI
pub const DEFAULT_ALLOWED_TOOLS: &[&str] = &[
    "Bash", "Read", "Edit", "Write", "Glob", "Grep", "Task", "TaskOutput",
    "TaskStop", "WebFetch", "WebSearch", "NotebookEdit", "Skill",
    "TaskCreate", "TaskGet", "TaskUpdate", "TaskList",
];

/// Execute a command using Claude CLI
pub fn execute_command(
    prompt: &str,
    session_id: Option<&str>,
    working_dir: &str,
    allowed_tools: Option<&[String]>,
) -> ClaudeResponse {
    let tools_str = match allowed_tools {
        Some(tools) => tools.join(","),
        None => DEFAULT_ALLOWED_TOOLS.join(","),
    };
    let mut args = vec![
        "-p".to_string(),
        "--allowedTools".to_string(),
        tools_str,
        "--output-format".to_string(),
        "json".to_string(),
        "--append-system-prompt".to_string(),
        DEFAULT_SYSTEM_PROMPT.to_string(),
    ];

    // Resume session if available
    if let Some(sid) = session_id {
        if !is_valid_session_id(sid) {
            return ClaudeResponse {
                success: false,
                response: None,
                session_id: None,
                error: Some("Invalid session ID format".to_string()),
            };
        }
        args.push("--resume".to_string());
        args.push(sid.to_string());
    }

    let claude_bin = match get_binary_path() {
        Some(path) => path,
        None => {
            return ClaudeResponse {
                success: false,
                response: None,
                session_id: None,
                error: Some("Claude CLI not found. Is Claude CLI installed?".to_string()),
            };
        }
    };

    let mut child = match Command::new(claude_bin)
        .args(&args)
        .current_dir(working_dir)
        .env("CLAUDE_CODE_MAX_OUTPUT_TOKENS", "64000")
        .env("BASH_DEFAULT_TIMEOUT_MS", "86400000")  // 24 hours (no practical timeout)
        .env("BASH_MAX_TIMEOUT_MS", "86400000")      // 24 hours (no practical timeout)
        .env_remove("CLAUDECODE")  // Allow running from within Claude Code sessions
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            return ClaudeResponse {
                success: false,
                response: None,
                session_id: None,
                error: Some(format!("Failed to start Claude: {}. Is Claude CLI installed?", e)),
            };
        }
    };

    // Write prompt to stdin
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(prompt.as_bytes());
    }

    // Wait for output
    match child.wait_with_output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                parse_claude_output(&stdout)
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                ClaudeResponse {
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
        Err(e) => ClaudeResponse {
            success: false,
            response: None,
            session_id: None,
            error: Some(format!("Failed to read output: {}", e)),
        },
    }
}

/// Parse Claude CLI JSON output
fn parse_claude_output(output: &str) -> ClaudeResponse {
    let mut session_id: Option<String> = None;
    let mut response_text = String::new();

    for line in output.trim().lines() {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            // Extract session ID
            if let Some(sid) = json.get("session_id").and_then(|v| v.as_str()) {
                session_id = Some(sid.to_string());
            }

            // Extract response text
            if let Some(result) = json.get("result").and_then(|v| v.as_str()) {
                response_text = result.to_string();
            } else if let Some(message) = json.get("message").and_then(|v| v.as_str()) {
                response_text = message.to_string();
            } else if let Some(content) = json.get("content").and_then(|v| v.as_str()) {
                response_text = content.to_string();
            }
        } else if !line.trim().is_empty() && !line.starts_with('{') {
            response_text.push_str(line);
            response_text.push('\n');
        }
    }

    // If no structured response, use raw output
    if response_text.is_empty() {
        response_text = output.trim().to_string();
    }

    ClaudeResponse {
        success: true,
        response: Some(response_text.trim().to_string()),
        session_id,
        error: None,
    }
}

/// Check if Claude CLI is available
pub fn is_claude_available() -> bool {
    is_cli_available()
}

/// Check if platform supports AI features
#[allow(dead_code)]
pub fn is_ai_supported() -> bool {
    cfg!(unix)
}

/// Execute a command using Claude CLI with streaming output
/// If `system_prompt` is None, uses the default file manager system prompt.
/// If `system_prompt` is Some(""), no system prompt is appended.
pub fn execute_command_streaming(
    prompt: &str,
    session_id: Option<&str>,
    working_dir: &str,
    sender: Sender<StreamMessage>,
    system_prompt: Option<&str>,
    allowed_tools: Option<&[String]>,
    cancel_token: Option<std::sync::Arc<CancelToken>>,
) -> Result<(), String> {
    debug_log(&format!("prompt_len: {} chars", prompt.len()));
    debug_log(&format!("session_id: {:?}", session_id));

    let tools_str = match allowed_tools {
        Some(tools) => tools.join(","),
        None => DEFAULT_ALLOWED_TOOLS.join(","),
    };
    let mut args = vec![
        "-p".to_string(),
        "--allowedTools".to_string(),
        tools_str,
        "--verbose".to_string(),
        "--output-format".to_string(),
        "stream-json".to_string(),
    ];

    // Append system prompt based on parameter
    let effective_prompt = match system_prompt {
        None => Some(DEFAULT_SYSTEM_PROMPT),
        Some("") => None,
        Some(p) => Some(p),
    };
    if let Some(sp) = effective_prompt {
        args.push("--append-system-prompt".to_string());
        args.push(sp.to_string());
    }

    // Resume session if available
    if let Some(sid) = session_id {
        if !is_valid_session_id(sid) {
            debug_log("ERROR: Invalid session ID format");
            return Err("Invalid session ID format".to_string());
        }
        args.push("--resume".to_string());
        args.push(sid.to_string());
    }

    let binary_path = get_binary_path()
        .ok_or_else(|| {
            debug_log("ERROR: Claude CLI not found");
            "Claude CLI not found. Is Claude CLI installed?".to_string()
        })?;

    let config = StreamingConfig {
        provider_name: "claude",
        binary_path,
        args: &args,
        working_dir,
        env_vars: &[
            ("CLAUDE_CODE_MAX_OUTPUT_TOKENS", "64000"),
            ("BASH_DEFAULT_TIMEOUT_MS", "86400000"),
            ("BASH_MAX_TIMEOUT_MS", "86400000"),
        ],
        env_remove: &["CLAUDECODE"],
        stdin_data: Some(prompt.as_bytes()),
        send_synthetic_init: false, // Claude does not need synthetic Init
    };

    provider_common::run_streaming(
        &config,
        sender,
        cancel_token,
        provider_common::make_default_handler(parse_stream_message),
    )
}

/// Parse a stream-json line into a StreamMessage
fn parse_stream_message(json: &Value) -> Option<StreamMessage> {
    let msg_type = json.get("type")?.as_str()?;

    match msg_type {
        "system" => {
            // {"type":"system","subtype":"init","session_id":"..."}
            // {"type":"system","subtype":"task_notification","task_id":"...","status":"...","summary":"..."}
            let subtype = json.get("subtype").and_then(|v| v.as_str())?;
            match subtype {
                "init" => {
                    let session_id = json.get("session_id")?.as_str()?.to_string();
                    Some(StreamMessage::Init { session_id })
                }
                "task_notification" => {
                    let task_id = json.get("task_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let status = json.get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let summary = json.get("summary")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Some(StreamMessage::TaskNotification { task_id, status, summary })
                }
                _ => None
            }
        }
        "assistant" => {
            // {"type":"assistant","message":{"content":[{"type":"text","text":"..."}]}}
            // or {"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{...}}]}}
            let content = json.get("message")?.get("content")?.as_array()?;

            for item in content {
                let item_type = item.get("type")?.as_str()?;
                match item_type {
                    "text" => {
                        let text = item.get("text")?.as_str()?.to_string();
                        return Some(StreamMessage::Text { content: text });
                    }
                    "tool_use" => {
                        let name = item.get("name")?.as_str()?.to_string();
                        let input = item.get("input")
                            .map(|v| serde_json::to_string_pretty(v).unwrap_or_default())
                            .unwrap_or_default();
                        return Some(StreamMessage::ToolUse { name, input });
                    }
                    _ => {}
                }
            }
            None
        }
        "user" => {
            // {"type":"user","message":{"content":[{"type":"tool_result","content":"..." or [array]}]}}
            let content = json.get("message")?.get("content")?.as_array()?;

            for item in content {
                let item_type = item.get("type")?.as_str()?;
                if item_type == "tool_result" {
                    // content can be a string or an array of text items
                    let content_text = if let Some(s) = item.get("content").and_then(|v| v.as_str()) {
                        s.to_string()
                    } else if let Some(arr) = item.get("content").and_then(|v| v.as_array()) {
                        // Extract text from array: [{"type":"text","text":"..."},...]
                        arr.iter()
                            .filter_map(|v| v.get("text").and_then(|t| t.as_str()))
                            .collect::<Vec<_>>()
                            .join("\n")
                    } else {
                        String::new()
                    };
                    let is_error = item.get("is_error")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    return Some(StreamMessage::ToolResult { content: content_text, is_error });
                }
            }
            None
        }
        "result" => {
            // {"type":"result","subtype":"success","result":"...","session_id":"..."}
            let result = json.get("result")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let session_id = json.get("session_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            Some(StreamMessage::Done { result, session_id })
        }
        _ => None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== is_valid_session_id tests ==========

    #[test]
    fn test_session_id_valid() {
        assert!(is_valid_session_id("abc123"));
        assert!(is_valid_session_id("session-1"));
        assert!(is_valid_session_id("session_2"));
        assert!(is_valid_session_id("ABC-XYZ_123"));
        assert!(is_valid_session_id("a")); // Single char
    }

    #[test]
    fn test_session_id_empty_rejected() {
        assert!(!is_valid_session_id(""));
    }

    #[test]
    fn test_session_id_too_long_rejected() {
        // 64 characters should be valid
        let max_len = "a".repeat(64);
        assert!(is_valid_session_id(&max_len));

        // 65 characters should be rejected
        let too_long = "a".repeat(65);
        assert!(!is_valid_session_id(&too_long));
    }

    #[test]
    fn test_session_id_special_chars_rejected() {
        assert!(!is_valid_session_id("session;rm -rf"));
        assert!(!is_valid_session_id("session'OR'1=1"));
        assert!(!is_valid_session_id("session`cmd`"));
        assert!(!is_valid_session_id("session$(cmd)"));
        assert!(!is_valid_session_id("session\nline2"));
        assert!(!is_valid_session_id("session\0null"));
        assert!(!is_valid_session_id("path/traversal"));
        assert!(!is_valid_session_id("session with space"));
        assert!(!is_valid_session_id("session.dot"));
        assert!(!is_valid_session_id("session@email"));
    }

    #[test]
    fn test_session_id_unicode_rejected() {
        assert!(!is_valid_session_id("세션아이디"));
        assert!(!is_valid_session_id("session_日本語"));
        assert!(!is_valid_session_id("émoji🎉"));
    }

    // ========== ClaudeResponse tests ==========

    #[test]
    fn test_claude_response_struct() {
        let response = ClaudeResponse {
            success: true,
            response: Some("Hello".to_string()),
            session_id: Some("abc123".to_string()),
            error: None,
        };

        assert!(response.success);
        assert_eq!(response.response, Some("Hello".to_string()));
        assert_eq!(response.session_id, Some("abc123".to_string()));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_claude_response_error() {
        let response = ClaudeResponse {
            success: false,
            response: None,
            session_id: None,
            error: Some("Connection failed".to_string()),
        };

        assert!(!response.success);
        assert!(response.response.is_none());
        assert_eq!(response.error, Some("Connection failed".to_string()));
    }

    // ========== parse_claude_output tests ==========

    #[test]
    fn test_parse_claude_output_json_result() {
        let output = r#"{"session_id": "test-123", "result": "Hello, world!"}"#;
        let response = parse_claude_output(output);

        assert!(response.success);
        assert_eq!(response.response, Some("Hello, world!".to_string()));
        assert_eq!(response.session_id, Some("test-123".to_string()));
    }

    #[test]
    fn test_parse_claude_output_json_message() {
        let output = r#"{"session_id": "sess-456", "message": "This is a message"}"#;
        let response = parse_claude_output(output);

        assert!(response.success);
        assert_eq!(response.response, Some("This is a message".to_string()));
    }

    #[test]
    fn test_parse_claude_output_plain_text() {
        let output = "Just plain text response";
        let response = parse_claude_output(output);

        assert!(response.success);
        assert_eq!(response.response, Some("Just plain text response".to_string()));
    }

    #[test]
    fn test_parse_claude_output_multiline() {
        let output = r#"{"session_id": "s1"}
{"result": "Final result"}"#;
        let response = parse_claude_output(output);

        assert!(response.success);
        assert_eq!(response.session_id, Some("s1".to_string()));
        assert_eq!(response.response, Some("Final result".to_string()));
    }

    #[test]
    fn test_parse_claude_output_empty() {
        let output = "";
        let response = parse_claude_output(output);

        assert!(response.success);
        // Empty output should return empty response
        assert_eq!(response.response, Some("".to_string()));
    }

    // ========== is_ai_supported tests ==========

    #[test]
    fn test_is_ai_supported() {
        #[cfg(unix)]
        assert!(is_ai_supported());

        #[cfg(not(unix))]
        assert!(!is_ai_supported());
    }

    // ========== session_id_regex tests ==========

    #[test]
    fn test_session_id_regex_caching() {
        // Multiple calls should return the same cached regex
        let regex1 = session_id_regex();
        let regex2 = session_id_regex();

        // Both should point to the same static instance
        assert!(std::ptr::eq(regex1, regex2));
    }

    // ========== parse_stream_message tests ==========

    #[test]
    fn test_parse_stream_message_init() {
        let json: Value = serde_json::from_str(
            r#"{"type":"system","subtype":"init","session_id":"test-123"}"#
        ).unwrap();

        match parse_stream_message(&json) {
            Some(StreamMessage::Init { session_id }) => {
                assert_eq!(session_id, "test-123");
            }
            _ => panic!("Expected Init message"),
        }
    }

    #[test]
    fn test_parse_stream_message_text() {
        let json: Value = serde_json::from_str(
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello world"}]}}"#
        ).unwrap();

        match parse_stream_message(&json) {
            Some(StreamMessage::Text { content }) => {
                assert_eq!(content, "Hello world");
            }
            _ => panic!("Expected Text message"),
        }
    }

    #[test]
    fn test_parse_stream_message_tool_use() {
        let json: Value = serde_json::from_str(
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"ls"}}]}}"#
        ).unwrap();

        match parse_stream_message(&json) {
            Some(StreamMessage::ToolUse { name, input }) => {
                assert_eq!(name, "Bash");
                assert!(input.contains("ls"));
            }
            _ => panic!("Expected ToolUse message"),
        }
    }

    #[test]
    fn test_parse_stream_message_tool_result() {
        let json: Value = serde_json::from_str(
            r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"file.txt","is_error":false}]}}"#
        ).unwrap();

        match parse_stream_message(&json) {
            Some(StreamMessage::ToolResult { content, is_error }) => {
                assert_eq!(content, "file.txt");
                assert!(!is_error);
            }
            _ => panic!("Expected ToolResult message"),
        }
    }

    #[test]
    fn test_parse_stream_message_tool_result_error() {
        let json: Value = serde_json::from_str(
            r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"Error: not found","is_error":true}]}}"#
        ).unwrap();

        match parse_stream_message(&json) {
            Some(StreamMessage::ToolResult { content, is_error }) => {
                assert_eq!(content, "Error: not found");
                assert!(is_error);
            }
            _ => panic!("Expected ToolResult message with error"),
        }
    }

    #[test]
    fn test_parse_stream_message_result() {
        let json: Value = serde_json::from_str(
            r#"{"type":"result","subtype":"success","result":"Done!","session_id":"sess-456"}"#
        ).unwrap();

        match parse_stream_message(&json) {
            Some(StreamMessage::Done { result, session_id }) => {
                assert_eq!(result, "Done!");
                assert_eq!(session_id, Some("sess-456".to_string()));
            }
            _ => panic!("Expected Done message"),
        }
    }

    #[test]
    fn test_parse_stream_message_unknown_type() {
        let json: Value = serde_json::from_str(
            r#"{"type":"unknown","data":"something"}"#
        ).unwrap();

        let msg = parse_stream_message(&json);
        assert!(msg.is_none());
    }
}
