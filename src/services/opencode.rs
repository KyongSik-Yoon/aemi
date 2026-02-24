use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc::Sender;
use std::sync::OnceLock;
use std::fs::OpenOptions;
use serde_json::Value;

pub use super::agent::{StreamMessage, CancelToken, AgentResponse};

/// Cached path to the opencode binary.
/// Once resolved, reused for all subsequent calls.
static OPENCODE_PATH: OnceLock<Option<String>> = OnceLock::new();

/// Resolve the path to the opencode binary.
/// First tries `which opencode`, then falls back to `bash -lc "which opencode"`
/// (for non-interactive SSH sessions where ~/.profile isn't loaded).
fn resolve_opencode_path() -> Option<String> {
    // Try direct `which opencode` first
    if let Ok(output) = Command::new("which").arg("opencode").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }

    // Fallback: use login shell to resolve PATH
    if let Ok(output) = Command::new("bash")
        .args(["-lc", "which opencode"])
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }

    None
}

/// Get the cached opencode binary path, resolving it on first call.
fn get_opencode_path() -> Option<&'static str> {
    OPENCODE_PATH.get_or_init(|| resolve_opencode_path()).as_deref()
}

/// Debug logging helper (only active when AIMI_DEBUG=1)
fn debug_log(msg: &str) {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    let enabled = ENABLED.get_or_init(|| {
        std::env::var("AIMI_DEBUG").map(|v| v == "1").unwrap_or(false)
    });
    if !*enabled { return; }
    if let Some(home) = dirs::home_dir() {
        let debug_dir = home.join(".aimi").join("debug");
        let _ = std::fs::create_dir_all(&debug_dir);
        let log_path = debug_dir.join("opencode.log");
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
        {
            let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
            let _ = writeln!(file, "[{}] {}", timestamp, msg);
        }
    }
}

/// Check if OpenCode CLI is available
pub fn is_opencode_available() -> bool {
    #[cfg(not(unix))]
    {
        false
    }

    #[cfg(unix)]
    {
        get_opencode_path().is_some()
    }
}

/// Execute a command using OpenCode CLI (non-streaming)
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

    let opencode_bin = match get_opencode_path() {
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
    debug_log("========================================");
    debug_log("=== opencode execute_command_streaming START ===");
    debug_log("========================================");
    debug_log(&format!("prompt_len: {} chars", prompt.len()));
    debug_log(&format!("working_dir: {}", working_dir));
    debug_log(&format!("session_id: {:?}", session_id));

    let default_system_prompt = r#"You are a terminal file manager assistant. Be concise. Focus on file operations. Respond in the same language as the user.

SECURITY RULES (MUST FOLLOW):
- NEVER execute destructive commands like rm -rf, format, mkfs, dd, etc.
- NEVER modify system files in /etc, /sys, /proc, /boot
- NEVER access or modify files outside the current working directory without explicit user path
- NEVER execute commands that could harm the system or compromise security
- ONLY suggest safe file operations: copy, move, rename, create directory, view, edit
- If a request seems dangerous, explain the risk and suggest a safer alternative

BASH EXECUTION RULES (MUST FOLLOW):
- All commands MUST run non-interactively without user input
- Use -y, --yes, or --non-interactive flags (e.g., apt install -y, npm init -y)
- Use -m flag for commit messages (e.g., git commit -m "message")
- Disable pagers with --no-pager or pipe to cat (e.g., git --no-pager log)
- NEVER use commands that open editors (vim, nano, etc.)
- NEVER use commands that wait for stdin without arguments
- NEVER use interactive flags like -i

IMPORTANT: Format your responses using Markdown for better readability:
- Use **bold** for important terms or commands
- Use `code` for file paths, commands, and technical terms
- Use bullet lists (- item) for multiple items
- Use numbered lists (1. item) for sequential steps
- Use code blocks (```language) for multi-line code or command examples
- Use headers (## Title) to organize longer responses
- Keep formatting minimal and terminal-friendly"#;

    // Build the effective prompt with system prompt prepended
    let effective_prompt = match system_prompt {
        None => format!("[System Instructions]\n{}\n\n[User Message]\n{}", default_system_prompt, prompt),
        Some("") => prompt.to_string(),
        Some(sp) => format!("[System Instructions]\n{}\n\n[User Message]\n{}", sp, prompt),
    };

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

    let opencode_bin = get_opencode_path()
        .ok_or_else(|| {
            debug_log("ERROR: OpenCode CLI not found");
            "OpenCode CLI not found. Is OpenCode CLI installed?".to_string()
        })?;

    debug_log("--- Spawning opencode process ---");
    debug_log(&format!("Command: {} {}", opencode_bin, args.iter()
        .map(|a| if a.len() > 50 { format!("{}...", &a[..50]) } else { a.clone() })
        .collect::<Vec<_>>().join(" ")));

    let spawn_start = std::time::Instant::now();
    let mut child = Command::new(opencode_bin)
        .args(&args)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            debug_log(&format!("ERROR: Failed to spawn: {}", e));
            format!("Failed to start OpenCode: {}. Is OpenCode CLI installed?", e)
        })?;
    debug_log(&format!("OpenCode process spawned in {:?}, pid={:?}", spawn_start.elapsed(), child.id()));

    // Store child PID in cancel token so the caller can kill it externally
    if let Some(ref token) = cancel_token {
        *token.child_pid.lock().unwrap() = Some(child.id());
    }

    // Read stdout line by line for streaming
    let stdout = child.stdout.take()
        .ok_or_else(|| "Failed to capture stdout".to_string())?;
    let reader = BufReader::new(stdout);

    let mut captured_session_id: Option<String> = None;
    let mut sent_init = false;
    let mut final_result: Option<String> = None;
    let mut line_count = 0;

    for line in reader.lines() {
        // Check cancel token before processing each line
        if let Some(ref token) = cancel_token {
            if token.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                debug_log("Cancel detected — killing child process");
                let _ = child.kill();
                let _ = child.wait();
                return Ok(());
            }
        }

        let line = match line {
            Ok(l) => l,
            Err(e) => {
                debug_log(&format!("ERROR: Failed to read line: {}", e));
                let _ = sender.send(StreamMessage::Error {
                    message: format!("Failed to read output: {}", e)
                });
                break;
            }
        };

        line_count += 1;

        if line.trim().is_empty() {
            continue;
        }

        debug_log(&format!("Line {}: {}", line_count, &line.chars().take(200).collect::<String>()));

        if let Ok(json) = serde_json::from_str::<Value>(&line) {
            // Capture sessionID from the first event
            if captured_session_id.is_none() {
                captured_session_id = json.get("sessionID")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }

            // Send Init on first event with sessionID
            if !sent_init {
                if let Some(ref sid) = captured_session_id {
                    let _ = sender.send(StreamMessage::Init { session_id: sid.clone() });
                    sent_init = true;
                }
            }

            if let Some(msg) = parse_stream_message(&json) {
                // Track final result
                if let StreamMessage::Done { ref result, .. } = msg {
                    final_result = Some(result.clone());
                }

                let send_result = sender.send(msg);
                if send_result.is_err() {
                    debug_log("Channel send failed (receiver dropped)");
                    break;
                }
            }
        }
    }

    // Check cancel token after exiting the loop
    if let Some(ref token) = cancel_token {
        if token.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(());
        }
    }

    // Wait for process to finish
    let status = child.wait().map_err(|e| format!("Process error: {}", e))?;
    debug_log(&format!("Process finished, exit_code: {:?}", status.code()));

    // If we never got an Init, send a synthetic one
    if !sent_init {
        let synthetic_id = format!("opencode-{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis());
        let _ = sender.send(StreamMessage::Init { session_id: synthetic_id });
    }

    // If we didn't get a proper Done message, send one now
    if final_result.is_none() {
        let _ = sender.send(StreamMessage::Done {
            result: String::new(),
            session_id: captured_session_id,
        });
    }

    if !status.success() {
        return Err(format!("Process exited with code {:?}", status.code()));
    }

    debug_log("=== opencode execute_command_streaming END (success) ===");
    Ok(())
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
