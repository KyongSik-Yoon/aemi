/// Shared infrastructure for AI provider streaming implementations.
///
/// Eliminates duplication across gemini.rs, claude.rs, codex.rs, opencode.rs
/// by centralizing the default system prompt, effective prompt building,
/// process spawning, streaming read loop, cancellation, and finalization.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, OnceLock};
use serde_json::Value;

use super::agent::{StreamMessage, CancelToken};

// ---------------------------------------------------------------------------
// Default system prompt (shared across all providers)
// ---------------------------------------------------------------------------

pub const DEFAULT_SYSTEM_PROMPT: &str = r#"You are a terminal file manager assistant. Be concise. Focus on file operations. Respond in the same language as the user.

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

// ---------------------------------------------------------------------------
// Effective prompt builder (for providers that embed system prompt in prompt text)
// ---------------------------------------------------------------------------

/// Build the effective prompt with system prompt prepended.
/// Used by gemini, codex, opencode (not claude, which uses --append-system-prompt).
pub fn build_effective_prompt(system_prompt: Option<&str>, prompt: &str) -> String {
    match system_prompt {
        None => format!("[System Instructions]\n{}\n\n[User Message]\n{}", DEFAULT_SYSTEM_PROMPT, prompt),
        Some("") => prompt.to_string(),
        Some(sp) => format!("[System Instructions]\n{}\n\n[User Message]\n{}", sp, prompt),
    }
}

// ---------------------------------------------------------------------------
// Debug logging (provider-name aware, for use in shared code)
// ---------------------------------------------------------------------------

pub fn debug_log_for(provider_name: &str, msg: &str) {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    let enabled = ENABLED.get_or_init(|| {
        std::env::var("AEMI_DEBUG").map(|v| v == "1").unwrap_or(false)
    });
    if !*enabled { return; }
    if let Some(home) = dirs::home_dir() {
        let debug_dir = home.join(".aemi").join("debug");
        let _ = std::fs::create_dir_all(&debug_dir);
        let log_path = debug_dir.join(format!("{}.log", provider_name));
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
        {
            let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
            let _ = writeln!(file, "[{}] {}", timestamp, msg);
        }
    }
}

// ---------------------------------------------------------------------------
// Streaming infrastructure
// ---------------------------------------------------------------------------

/// Configuration for spawning a provider CLI process.
pub struct StreamingConfig<'a> {
    pub provider_name: &'a str,
    pub binary_path: &'a str,
    pub args: &'a [String],
    pub working_dir: &'a str,
    /// Extra environment variables to set on the child process.
    pub env_vars: &'a [(&'a str, &'a str)],
    /// Environment variables to remove from the child process.
    pub env_remove: &'a [&'a str],
    /// If Some, write this data to the child's stdin before reading stdout.
    pub stdin_data: Option<&'a [u8]>,
    /// Whether to send a synthetic Init message if none was received from the CLI.
    pub send_synthetic_init: bool,
}

/// Mutable state tracked during the streaming read loop.
pub struct StreamState {
    pub session_id: Option<String>,
    pub sent_init: bool,
    pub final_result: Option<String>,
}

impl StreamState {
    pub fn new() -> Self {
        Self {
            session_id: None,
            sent_init: false,
            final_result: None,
        }
    }
}

/// Process a parsed [`StreamMessage`]: update tracking state and send to channel.
///
/// Returns `true` if the loop should continue, `false` if the channel is closed.
pub fn handle_parsed_message(
    msg: StreamMessage,
    sender: &Sender<StreamMessage>,
    state: &mut StreamState,
) -> bool {
    if let StreamMessage::Init { ref session_id } = msg {
        state.session_id = Some(session_id.clone());
        state.sent_init = true;
    }
    if let StreamMessage::Done { ref result, ref session_id } = msg {
        state.final_result = Some(result.clone());
        if session_id.is_some() {
            state.session_id = session_id.clone();
        }
    }
    sender.send(msg).is_ok()
}

/// Create a default JSON handler that parses each line with `parse_fn`,
/// then feeds the result through [`handle_parsed_message`].
pub fn make_default_handler<P>(
    parse_fn: P,
) -> impl FnMut(&Value, &Sender<StreamMessage>, &mut StreamState) -> bool
where
    P: Fn(&Value) -> Option<StreamMessage>,
{
    move |json, sender, state| {
        if let Some(msg) = parse_fn(json) {
            return handle_parsed_message(msg, sender, state);
        }
        true
    }
}

/// Spawn a provider CLI, stream its stdout line-by-line, and finalize.
///
/// `handle_json` is called for every successfully-parsed JSON line.
/// It should return `true` to continue or `false` to break (channel closed).
pub fn run_streaming<F>(
    config: &StreamingConfig,
    sender: Sender<StreamMessage>,
    cancel_token: Option<Arc<CancelToken>>,
    mut handle_json: F,
) -> Result<(), String>
where
    F: FnMut(&Value, &Sender<StreamMessage>, &mut StreamState) -> bool,
{
    let log = |msg: &str| debug_log_for(config.provider_name, msg);

    log("========================================");
    log(&format!("=== {} execute_command_streaming START ===", config.provider_name));
    log("========================================");
    log(&format!("working_dir: {}", config.working_dir));

    // --- Spawn child process ---
    log(&format!("--- Spawning {} process ---", config.provider_name));
    log(&format!("Command: {}", config.binary_path));

    let spawn_start = std::time::Instant::now();
    let mut cmd = Command::new(config.binary_path);
    cmd.args(config.args)
       .current_dir(config.working_dir)
       .stdout(Stdio::piped())
       .stderr(Stdio::piped());

    for &(key, val) in config.env_vars {
        cmd.env(key, val);
    }
    for &key in config.env_remove {
        cmd.env_remove(key);
    }
    if config.stdin_data.is_some() {
        cmd.stdin(Stdio::piped());
    }

    let mut child = cmd.spawn().map_err(|e| {
        log(&format!("ERROR: Failed to spawn: {}", e));
        format!("Failed to start {}: {}. Is {} CLI installed?",
                config.provider_name, e, config.provider_name)
    })?;
    log(&format!("{} process spawned in {:?}, pid={:?}",
                 config.provider_name, spawn_start.elapsed(), child.id()));

    // Store child PID in cancel token so the caller can kill it externally
    if let Some(ref token) = cancel_token {
        *token.child_pid.lock().unwrap() = Some(child.id());
    }

    // Write to stdin if needed
    if let Some(data) = config.stdin_data {
        if let Some(mut stdin) = child.stdin.take() {
            log(&format!("Writing to stdin ({} bytes)...", data.len()));
            let _ = stdin.write_all(data);
            log("stdin handle dropped (closed)");
        }
    }

    // Take stderr handle before reading stdout so we can report CLI errors
    let stderr_handle = child.stderr.take();

    // Set up stdout reader
    let stdout = child.stdout.take()
        .ok_or_else(|| "Failed to capture stdout".to_string())?;
    let reader = BufReader::new(stdout);

    let mut state = StreamState::new();
    let mut line_count: u64 = 0;

    // --- Streaming read loop ---
    for line in reader.lines() {
        if check_cancelled(&cancel_token, &mut child, config.provider_name) {
            return Ok(());
        }

        let line = match line {
            Ok(l) => l,
            Err(e) => {
                log(&format!("ERROR: Failed to read line: {}", e));
                let _ = sender.send(StreamMessage::Error {
                    message: format!("Failed to read output: {}", e),
                });
                break;
            }
        };

        line_count += 1;

        if line.trim().is_empty() {
            continue;
        }

        log(&format!("Line {}: {}", line_count, &line.chars().take(200).collect::<String>()));

        if let Ok(json) = serde_json::from_str::<Value>(&line) {
            if !handle_json(&json, &sender, &mut state) {
                log("Channel send failed (receiver dropped)");
                break;
            }
        }
    }

    log(&format!("Read loop finished, total lines: {}", line_count));

    // --- Finalization ---
    if check_cancelled(&cancel_token, &mut child, config.provider_name) {
        return Ok(());
    }

    let status = child.wait().map_err(|e| format!("Process error: {}", e))?;
    log(&format!("Process finished, exit_code: {:?}", status.code()));

    // Send synthetic Init if the CLI never sent one
    if config.send_synthetic_init && !state.sent_init {
        let synthetic_id = format!("{}-{}", config.provider_name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis());
        let _ = sender.send(StreamMessage::Init { session_id: synthetic_id });
    }

    // Send synthetic Done if we didn't get a proper one
    if state.final_result.is_none() {
        let _ = sender.send(StreamMessage::Done {
            result: String::new(),
            session_id: state.session_id,
        });
    }

    if !status.success() {
        let stderr_msg = read_stderr(stderr_handle);
        return Err(match stderr_msg {
            Some(msg) => msg,
            None => format!("Process exited with code {:?}", status.code()),
        });
    }

    log(&format!("=== {} execute_command_streaming END (success) ===", config.provider_name));
    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn check_cancelled(
    cancel_token: &Option<Arc<CancelToken>>,
    child: &mut Child,
    provider_name: &str,
) -> bool {
    if let Some(ref token) = cancel_token {
        if token.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            debug_log_for(provider_name, "Cancel detected — killing child process");
            let _ = child.kill();
            let _ = child.wait();
            return true;
        }
    }
    false
}

fn read_stderr(stderr_handle: Option<std::process::ChildStderr>) -> Option<String> {
    stderr_handle.and_then(|h| {
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut BufReader::new(h), &mut buf).ok()?;
        let trimmed = buf.trim().to_string();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_effective_prompt_with_default() {
        let result = build_effective_prompt(None, "hello");
        assert!(result.contains("[System Instructions]"));
        assert!(result.contains(DEFAULT_SYSTEM_PROMPT));
        assert!(result.contains("[User Message]"));
        assert!(result.contains("hello"));
    }

    #[test]
    fn test_build_effective_prompt_empty_system() {
        let result = build_effective_prompt(Some(""), "hello");
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_build_effective_prompt_custom_system() {
        let result = build_effective_prompt(Some("Custom instructions"), "hello");
        assert!(result.contains("Custom instructions"));
        assert!(result.contains("hello"));
        assert!(!result.contains(DEFAULT_SYSTEM_PROMPT));
    }

    #[test]
    fn test_handle_parsed_message_init() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut state = StreamState::new();
        let msg = StreamMessage::Init { session_id: "test-123".to_string() };

        let ok = handle_parsed_message(msg, &tx, &mut state);
        assert!(ok);
        assert_eq!(state.session_id, Some("test-123".to_string()));
        assert!(state.sent_init);

        match rx.recv().unwrap() {
            StreamMessage::Init { session_id } => assert_eq!(session_id, "test-123"),
            _ => panic!("Expected Init"),
        }
    }

    #[test]
    fn test_handle_parsed_message_done() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut state = StreamState::new();
        let msg = StreamMessage::Done {
            result: "done".to_string(),
            session_id: Some("s1".to_string()),
        };

        let ok = handle_parsed_message(msg, &tx, &mut state);
        assert!(ok);
        assert_eq!(state.final_result, Some("done".to_string()));
        assert_eq!(state.session_id, Some("s1".to_string()));

        match rx.recv().unwrap() {
            StreamMessage::Done { result, session_id } => {
                assert_eq!(result, "done");
                assert_eq!(session_id, Some("s1".to_string()));
            }
            _ => panic!("Expected Done"),
        }
    }

    #[test]
    fn test_handle_parsed_message_text() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut state = StreamState::new();
        let msg = StreamMessage::Text { content: "hello".to_string() };

        let ok = handle_parsed_message(msg, &tx, &mut state);
        assert!(ok);
        // Text doesn't change state
        assert!(state.session_id.is_none());
        assert!(!state.sent_init);
        assert!(state.final_result.is_none());

        match rx.recv().unwrap() {
            StreamMessage::Text { content } => assert_eq!(content, "hello"),
            _ => panic!("Expected Text"),
        }
    }
}
