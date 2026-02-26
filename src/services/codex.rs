use std::process::Stdio;
use std::sync::mpsc::Sender;
use serde_json::Value;

pub use super::agent::{StreamMessage, CancelToken, AgentResponse};
use super::provider_common::{self, StreamingConfig};

// Generate resolve_binary_path(), get_binary_path(), is_cli_available(), debug_log() for "codex"
define_ai_service_helpers!("codex");

/// Check if Codex CLI is available
#[allow(dead_code)]
pub fn is_codex_available() -> bool {
    is_cli_available()
}

/// Execute a command using Codex CLI (non-streaming)
#[allow(dead_code)]
pub fn execute_command(
    prompt: &str,
    session_id: Option<&str>,
    working_dir: &str,
) -> AgentResponse {
    let mut args = vec![
        "exec".to_string(),
        "--json".to_string(),
        "--full-auto".to_string(),
    ];

    // Note: Codex CLI does not support --resume for session continuity.
    // session_id is accepted for API compatibility but not passed to the CLI.
    let _ = session_id;

    // Prompt as positional argument (last)
    args.push(prompt.to_string());

    let codex_bin = match get_binary_path() {
        Some(path) => path,
        None => {
            return AgentResponse {
                success: false,
                response: None,
                session_id: None,
                error: Some("Codex CLI not found. Is Codex CLI installed?".to_string()),
            };
        }
    };

    let child = match Command::new(codex_bin)
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
                error: Some(format!("Failed to start Codex: {}. Is Codex CLI installed?", e)),
            };
        }
    };

    // Wait for output — parse JSONL and extract last agent_message
    match child.wait_with_output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                parse_codex_jsonl_output(&stdout)
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

/// Parse Codex CLI JSONL output: extract thread_id and last agent_message text
#[allow(dead_code)]
fn parse_codex_jsonl_output(output: &str) -> AgentResponse {
    let mut thread_id: Option<String> = None;
    let mut last_text: Option<String> = None;

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Ok(json) = serde_json::from_str::<Value>(line) {
            let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match msg_type {
                "thread.started" => {
                    thread_id = json.get("thread_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
                "item.completed" => {
                    if let Some(item) = json.get("item") {
                        let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                        if item_type == "agent_message" {
                            last_text = item.get("text")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    AgentResponse {
        success: last_text.is_some(),
        response: last_text,
        session_id: thread_id,
        error: None,
    }
}

/// Execute a command using Codex CLI with streaming output
pub fn execute_command_streaming(
    prompt: &str,
    session_id: Option<&str>,
    working_dir: &str,
    sender: Sender<StreamMessage>,
    system_prompt: Option<&str>,
    _allowed_tools: Option<&[String]>, // Codex uses --full-auto instead of tool allowlist
    cancel_token: Option<std::sync::Arc<CancelToken>>,
) -> Result<(), String> {
    debug_log(&format!("prompt_len: {} chars", prompt.len()));
    debug_log(&format!("session_id: {:?}", session_id));

    let effective_prompt = provider_common::build_effective_prompt(system_prompt, prompt);

    // Build args: codex exec --json --full-auto [--resume <session_id>] "prompt"
    let mut args = vec![
        "exec".to_string(),
        "--json".to_string(),
        "--full-auto".to_string(),
    ];

    // Note: Codex CLI does not support --resume for session continuity.
    // Session tracking is handled at the aemi level only.
    if session_id.is_some() {
        debug_log("Session ID available but Codex CLI does not support --resume, skipping");
    }

    // Prompt as positional argument (must be last)
    args.push(effective_prompt);

    let binary_path = get_binary_path()
        .ok_or_else(|| {
            debug_log("ERROR: Codex CLI not found");
            "Codex CLI not found. Is Codex CLI installed?".to_string()
        })?;

    let config = StreamingConfig {
        provider_name: "codex",
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

/// Parse a Codex exec --json line into a StreamMessage.
///
/// Codex JSONL event types:
/// - thread.started: session initialization with thread_id
/// - turn.started / turn.completed: turn lifecycle (skip / Done)
/// - turn.failed: turn-level error
/// - item.started: tool execution started (command_execution, mcp_tool_call)
/// - item.completed: text response or tool result (agent_message, command_execution, file_change, mcp_tool_call)
/// - error: stream-level error
fn parse_stream_message(json: &Value) -> Option<StreamMessage> {
    let msg_type = json.get("type")?.as_str()?;

    match msg_type {
        "thread.started" => {
            // {"type":"thread.started","thread_id":"uuid"}
            let thread_id = json.get("thread_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            debug_log(&format!("thread.started: {}", thread_id));
            Some(StreamMessage::Init { session_id: thread_id })
        }

        "item.started" => {
            // {"type":"item.started","item":{"id":"...","type":"command_execution","command":"ls"}}
            let item = json.get("item")?;
            let item_type = item.get("type")?.as_str()?;

            match item_type {
                "command_execution" => {
                    let command = item.get("command")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Some(StreamMessage::ToolUse {
                        name: "Bash".to_string(),
                        input: command,
                    })
                }
                "mcp_tool_call" => {
                    let tool = item.get("tool")
                        .and_then(|v| v.as_str())
                        .unwrap_or("mcp_tool")
                        .to_string();
                    let arguments = item.get("arguments")
                        .map(|v| serde_json::to_string_pretty(v).unwrap_or_default())
                        .unwrap_or_default();
                    Some(StreamMessage::ToolUse {
                        name: tool,
                        input: arguments,
                    })
                }
                _ => None
            }
        }

        "item.completed" => {
            // {"type":"item.completed","item":{"id":"...","type":"agent_message","text":"..."}}
            let item = json.get("item")?;
            let item_type = item.get("type")?.as_str()?;

            match item_type {
                "agent_message" => {
                    let text = item.get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    if text.is_empty() { return None; }
                    Some(StreamMessage::Text { content: text })
                }
                "command_execution" => {
                    let output = item.get("aggregated_output")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let exit_code = item.get("exit_code")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(-1);
                    Some(StreamMessage::ToolResult {
                        content: output,
                        is_error: exit_code != 0,
                    })
                }
                "file_change" => {
                    let changes = item.get("changes")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|c| {
                                    let path = c.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                                    let kind = c.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
                                    Some(format!("{}: {}", kind, path))
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                        .unwrap_or_default();
                    Some(StreamMessage::ToolResult {
                        content: changes,
                        is_error: false,
                    })
                }
                "mcp_tool_call" => {
                    let status = item.get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let is_error = status == "failed";
                    let content = if is_error {
                        item.get("error")
                            .map(|v| serde_json::to_string_pretty(v).unwrap_or_default())
                            .unwrap_or_default()
                    } else {
                        item.get("result")
                            .map(|v| serde_json::to_string_pretty(v).unwrap_or_default())
                            .unwrap_or_default()
                    };
                    Some(StreamMessage::ToolResult { content, is_error })
                }
                // "reasoning" — internal chain-of-thought, skip
                _ => None
            }
        }

        "turn.completed" => {
            // {"type":"turn.completed","usage":{"input_tokens":...,"output_tokens":...}}
            Some(StreamMessage::Done {
                result: String::new(),
                session_id: None,
            })
        }

        "turn.failed" => {
            let message = json.get("error")
                .and_then(|v| v.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("Turn failed")
                .to_string();
            Some(StreamMessage::Error { message })
        }

        "error" => {
            let message = json.get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error")
                .to_string();
            Some(StreamMessage::Error { message })
        }

        // turn.started, item.updated — skip
        _ => None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_codex_jsonl_output_success() {
        let output = r#"{"type":"thread.started","thread_id":"abc-123"}
{"type":"turn.started"}
{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"Hello, world!"}}
{"type":"turn.completed","usage":{"input_tokens":100,"output_tokens":10}}"#;
        let response = parse_codex_jsonl_output(output);
        assert!(response.success);
        assert_eq!(response.response, Some("Hello, world!".to_string()));
        assert_eq!(response.session_id, Some("abc-123".to_string()));
    }

    #[test]
    fn test_parse_codex_jsonl_output_empty() {
        let output = r#"{"type":"thread.started","thread_id":"abc-123"}
{"type":"turn.completed","usage":{}}"#;
        let response = parse_codex_jsonl_output(output);
        assert!(!response.success);
        assert!(response.response.is_none());
        assert_eq!(response.session_id, Some("abc-123".to_string()));
    }

    #[test]
    fn test_parse_stream_thread_started() {
        let json: Value = serde_json::from_str(
            r#"{"type":"thread.started","thread_id":"uuid-abc-123"}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::Init { session_id }) => assert_eq!(session_id, "uuid-abc-123"),
            _ => panic!("Expected Init message"),
        }
    }

    #[test]
    fn test_parse_stream_agent_message() {
        let json: Value = serde_json::from_str(
            r#"{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"Hello from Codex"}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::Text { content }) => assert_eq!(content, "Hello from Codex"),
            _ => panic!("Expected Text message"),
        }
    }

    #[test]
    fn test_parse_stream_command_started() {
        let json: Value = serde_json::from_str(
            r#"{"type":"item.started","item":{"id":"item_2","type":"command_execution","command":"ls -la","status":"in_progress"}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::ToolUse { name, input }) => {
                assert_eq!(name, "Bash");
                assert_eq!(input, "ls -la");
            }
            _ => panic!("Expected ToolUse message"),
        }
    }

    #[test]
    fn test_parse_stream_command_completed() {
        let json: Value = serde_json::from_str(
            r#"{"type":"item.completed","item":{"id":"item_2","type":"command_execution","command":"ls","aggregated_output":"file.txt\ndir/","exit_code":0,"status":"completed"}}"#
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
    fn test_parse_stream_command_failed() {
        let json: Value = serde_json::from_str(
            r#"{"type":"item.completed","item":{"id":"item_3","type":"command_execution","command":"false","aggregated_output":"","exit_code":1,"status":"completed"}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::ToolResult { is_error, .. }) => assert!(is_error),
            _ => panic!("Expected ToolResult message with error"),
        }
    }

    #[test]
    fn test_parse_stream_file_change() {
        let json: Value = serde_json::from_str(
            r#"{"type":"item.completed","item":{"id":"item_4","type":"file_change","changes":[{"path":"/src/main.rs","kind":"update"},{"path":"/new.txt","kind":"add"}]}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::ToolResult { content, is_error }) => {
                assert!(content.contains("update: /src/main.rs"));
                assert!(content.contains("add: /new.txt"));
                assert!(!is_error);
            }
            _ => panic!("Expected ToolResult message"),
        }
    }

    #[test]
    fn test_parse_stream_mcp_tool_started() {
        let json: Value = serde_json::from_str(
            r#"{"type":"item.started","item":{"id":"item_5","type":"mcp_tool_call","server":"srv","tool":"search","arguments":{"query":"test"},"status":"in_progress"}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::ToolUse { name, input }) => {
                assert_eq!(name, "search");
                assert!(input.contains("test"));
            }
            _ => panic!("Expected ToolUse message"),
        }
    }

    #[test]
    fn test_parse_stream_turn_completed() {
        let json: Value = serde_json::from_str(
            r#"{"type":"turn.completed","usage":{"input_tokens":100,"output_tokens":50}}"#
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
    fn test_parse_stream_turn_failed() {
        let json: Value = serde_json::from_str(
            r#"{"type":"turn.failed","error":{"message":"Rate limit exceeded"}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::Error { message }) => {
                assert_eq!(message, "Rate limit exceeded");
            }
            _ => panic!("Expected Error message"),
        }
    }

    #[test]
    fn test_parse_stream_error() {
        let json: Value = serde_json::from_str(
            r#"{"type":"error","message":"Connection lost"}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::Error { message }) => {
                assert_eq!(message, "Connection lost");
            }
            _ => panic!("Expected Error message"),
        }
    }

    #[test]
    fn test_parse_stream_turn_started_skipped() {
        let json: Value = serde_json::from_str(
            r#"{"type":"turn.started"}"#
        ).unwrap();
        assert!(parse_stream_message(&json).is_none());
    }

    #[test]
    fn test_parse_stream_reasoning_skipped() {
        let json: Value = serde_json::from_str(
            r#"{"type":"item.completed","item":{"id":"item_6","type":"reasoning","text":"thinking..."}}"#
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
