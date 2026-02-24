use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc::Sender;
use std::sync::OnceLock;
use std::fs::OpenOptions;
use serde_json::Value;

pub use super::agent::{StreamMessage, CancelToken, AgentResponse};

/// Cached path to the gemini binary.
/// Once resolved, reused for all subsequent calls.
static GEMINI_PATH: OnceLock<Option<String>> = OnceLock::new();

/// Resolve the path to the gemini binary.
/// First tries `which gemini`, then falls back to `bash -lc "which gemini"`
/// (for non-interactive SSH sessions where ~/.profile isn't loaded).
fn resolve_gemini_path() -> Option<String> {
    // Try direct `which gemini` first
    if let Ok(output) = Command::new("which").arg("gemini").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }

    // Fallback: use login shell to resolve PATH
    if let Ok(output) = Command::new("bash")
        .args(["-lc", "which gemini"])
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

/// Get the cached gemini binary path, resolving it on first call.
fn get_gemini_path() -> Option<&'static str> {
    GEMINI_PATH.get_or_init(|| resolve_gemini_path()).as_deref()
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
        let log_path = debug_dir.join("gemini.log");
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

/// Check if Gemini CLI is available
pub fn is_gemini_available() -> bool {
    #[cfg(not(unix))]
    {
        false
    }

    #[cfg(unix)]
    {
        get_gemini_path().is_some()
    }
}

/// Execute a command using Gemini CLI (non-streaming)
pub fn execute_command(
    prompt: &str,
    working_dir: &str,
) -> AgentResponse {
    let args = vec![
        "-p".to_string(),
        "--output-format".to_string(),
        "json".to_string(),
        "--yolo".to_string(),
    ];

    let gemini_bin = match get_gemini_path() {
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

    let mut child = match Command::new(gemini_bin)
        .args(&args)
        .current_dir(working_dir)
        .stdin(Stdio::piped())
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

    // Write prompt to stdin
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(prompt.as_bytes());
    }

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
    debug_log("========================================");
    debug_log("=== gemini execute_command_streaming START ===");
    debug_log("========================================");
    debug_log(&format!("prompt_len: {} chars", prompt.len()));
    debug_log(&format!("working_dir: {}", working_dir));

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

    let args = vec![
        "-p".to_string(),
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--yolo".to_string(),
    ];

    let gemini_bin = get_gemini_path()
        .ok_or_else(|| {
            debug_log("ERROR: Gemini CLI not found");
            "Gemini CLI not found. Is Gemini CLI installed?".to_string()
        })?;

    debug_log("--- Spawning gemini process ---");
    debug_log(&format!("Command: {}", gemini_bin));

    let spawn_start = std::time::Instant::now();
    let mut child = Command::new(gemini_bin)
        .args(&args)
        .current_dir(working_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            debug_log(&format!("ERROR: Failed to spawn: {}", e));
            format!("Failed to start Gemini: {}. Is Gemini CLI installed?", e)
        })?;
    debug_log(&format!("Gemini process spawned in {:?}, pid={:?}", spawn_start.elapsed(), child.id()));

    // Store child PID in cancel token so the caller can kill it externally
    if let Some(ref token) = cancel_token {
        *token.child_pid.lock().unwrap() = Some(child.id());
    }

    // Write prompt to stdin
    if let Some(mut stdin) = child.stdin.take() {
        debug_log(&format!("Writing prompt to stdin ({} bytes)...", effective_prompt.len()));
        let _ = stdin.write_all(effective_prompt.as_bytes());
        // stdin is dropped here, which closes it - this signals end of input to gemini
    }

    // Read stdout line by line for streaming
    let stdout = child.stdout.take()
        .ok_or_else(|| "Failed to capture stdout".to_string())?;
    let reader = BufReader::new(stdout);

    // Generate a synthetic session ID for aimi-level session tracking
    let synthetic_session_id = format!("gemini-{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis());

    // Send Init message with synthetic session ID
    let _ = sender.send(StreamMessage::Init {
        session_id: synthetic_session_id.clone(),
    });

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

    // If we didn't get a proper Done message, send one now
    if final_result.is_none() {
        let _ = sender.send(StreamMessage::Done {
            result: String::new(),
            session_id: Some(synthetic_session_id),
        });
    }

    if !status.success() {
        // Gemini exit codes: 1=error, 42=input error, 53=turn limit
        return Err(format!("Process exited with code {:?}", status.code()));
    }

    debug_log("=== gemini execute_command_streaming END (success) ===");
    Ok(())
}

/// Parse a Gemini stream-json line into a StreamMessage.
///
/// Gemini stream-json events:
/// - init: session metadata
/// - message (role=assistant): text chunks
/// - tool_use: tool call requests
/// - tool_result: tool execution output
/// - result: final outcome with stats
/// - error: non-fatal warnings
fn parse_stream_message(json: &Value) -> Option<StreamMessage> {
    let msg_type = json.get("type")?.as_str()?;

    match msg_type {
        "init" => {
            // Gemini init event — we already sent a synthetic Init,
            // but if Gemini provides one, use it for logging
            debug_log("Gemini init event received");
            None // Already sent synthetic Init
        }
        "message" => {
            // {"type":"message","role":"assistant","content":[{"type":"text","text":"..."}]}
            let role = json.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role != "assistant" {
                return None;
            }

            if let Some(content) = json.get("content").and_then(|v| v.as_array()) {
                for item in content {
                    let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    if item_type == "text" {
                        let text = item.get("text")?.as_str()?.to_string();
                        return Some(StreamMessage::Text { content: text });
                    }
                }
            }
            // Fallback: check for top-level "text" field
            if let Some(text) = json.get("text").and_then(|v| v.as_str()) {
                return Some(StreamMessage::Text { content: text.to_string() });
            }
            None
        }
        "tool_use" => {
            // {"type":"tool_use","name":"shell","input":{"command":"ls"}}
            let name = json.get("name")?.as_str()?.to_string();
            let input = json.get("input")
                .map(|v| serde_json::to_string_pretty(v).unwrap_or_default())
                .unwrap_or_default();
            Some(StreamMessage::ToolUse { name, input })
        }
        "tool_result" => {
            // {"type":"tool_result","content":"...","is_error":false}
            let content = if let Some(s) = json.get("content").and_then(|v| v.as_str()) {
                s.to_string()
            } else if let Some(arr) = json.get("content").and_then(|v| v.as_array()) {
                arr.iter()
                    .filter_map(|v| v.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                String::new()
            };
            let is_error = json.get("is_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Some(StreamMessage::ToolResult { content, is_error })
        }
        "result" => {
            // {"type":"result","response":"...","stats":{...}}
            let result = json.get("response")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Some(StreamMessage::Done { result, session_id: None })
        }
        "error" => {
            let message = json.get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error")
                .to_string();
            Some(StreamMessage::Error { message })
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
    fn test_parse_stream_message_text() {
        let json: Value = serde_json::from_str(
            r#"{"type":"message","role":"assistant","content":[{"type":"text","text":"Hello"}]}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::Text { content }) => assert_eq!(content, "Hello"),
            _ => panic!("Expected Text message"),
        }
    }

    #[test]
    fn test_parse_stream_message_tool_use() {
        let json: Value = serde_json::from_str(
            r#"{"type":"tool_use","name":"shell","input":{"command":"ls -la"}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::ToolUse { name, input }) => {
                assert_eq!(name, "shell");
                assert!(input.contains("ls -la"));
            }
            _ => panic!("Expected ToolUse message"),
        }
    }

    #[test]
    fn test_parse_stream_message_tool_result() {
        let json: Value = serde_json::from_str(
            r#"{"type":"tool_result","content":"file.txt","is_error":false}"#
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
    fn test_parse_stream_message_result() {
        let json: Value = serde_json::from_str(
            r#"{"type":"result","response":"All done!","stats":{}}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::Done { result, session_id }) => {
                assert_eq!(result, "All done!");
                assert!(session_id.is_none());
            }
            _ => panic!("Expected Done message"),
        }
    }

    #[test]
    fn test_parse_stream_message_error() {
        let json: Value = serde_json::from_str(
            r#"{"type":"error","message":"Rate limit exceeded"}"#
        ).unwrap();
        match parse_stream_message(&json) {
            Some(StreamMessage::Error { message }) => {
                assert_eq!(message, "Rate limit exceeded");
            }
            _ => panic!("Expected Error message"),
        }
    }

    #[test]
    fn test_parse_stream_message_user_message_ignored() {
        let json: Value = serde_json::from_str(
            r#"{"type":"message","role":"user","content":[{"type":"text","text":"ignored"}]}"#
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
