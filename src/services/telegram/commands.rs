use std::sync::atomic::Ordering;
use std::path::Path;
use std::fs;

use teloxide::prelude::*;
use teloxide::types::ParseMode;

use crate::services::session::{HistoryItem, HistoryType};
use crate::services::bot_common::{self, ALL_TOOLS, AVAILABLE_AGENTS, normalize_tool_name, tool_info, risk_badge, is_valid_agent};
use super::{ChatSession, SharedState, token_hash};
use super::messages::{shared_rate_limit_wait, send_long_message, html_escape};

/// Handle /help command
pub async fn handle_help_command(
    bot: &Bot,
    chat_id: ChatId,
    state: &SharedState,
) -> ResponseResult<()> {
    let help = "\
<b>aemi Telegram Bot</b>
Manage server files &amp; chat with Claude AI.

<b>Session</b>
<code>/start &lt;path&gt;</code> — Start session at directory
<code>/start</code> — Start with auto-generated workspace
<code>/resume</code> — List &amp; resume saved sessions
<code>/pwd</code> — Show current working directory
<code>/clear</code> — Clear AI conversation history
<code>/stop</code> — Stop current AI request

<b>File Transfer</b>
<code>/down &lt;file&gt;</code> — Download file from server
Send a file/photo — Upload to session directory

<b>Shell</b>
<code>!&lt;command&gt;</code> — Run shell command directly
  e.g. <code>!ls -la</code>, <code>!git status</code>

<b>AI Chat</b>
Any other message is sent to AI agent.
AI can read, edit, and run commands in your session.

<b>Agent</b>
<code>/agent</code> — Show current AI agent
<code>/agent &lt;name&gt;</code> — Switch agent (claude, gemini, codex, opencode)

<b>Tool Management</b>
<code>/availabletools</code> — List all available tools
<code>/allowedtools</code> — Show currently allowed tools
<code>/allowed +name</code> — Add tool (e.g. <code>/allowed +Bash</code>)
<code>/allowed -name</code> — Remove tool

<code>/help</code> — Show this help";

    shared_rate_limit_wait(state, chat_id).await;
    bot.send_message(chat_id, help)
        .parse_mode(ParseMode::Html)
        .await?;

    Ok(())
}

/// Handle /start <path> command
pub async fn handle_start_command(
    bot: &Bot,
    chat_id: ChatId,
    text: &str,
    state: &SharedState,
    token: &str,
) -> ResponseResult<()> {
    // Extract path from "/start <path>"
    let path_str = text.strip_prefix("/start").unwrap_or("").trim();

    let canonical_path = if path_str.is_empty() {
        // Create random workspace directory
        let Some(home) = dirs::home_dir() else {
            shared_rate_limit_wait(state, chat_id).await;
            bot.send_message(chat_id, "Error: cannot determine home directory.")
                .await?;
            return Ok(());
        };
        let workspace_dir = home.join(".aemi").join("workspace");
        use rand::Rng;
        let random_name: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(8)
            .map(|b| (b as char).to_ascii_lowercase())
            .collect();
        let new_dir = workspace_dir.join(&random_name);
        if let Err(e) = fs::create_dir_all(&new_dir) {
            shared_rate_limit_wait(state, chat_id).await;
            bot.send_message(chat_id, format!("Error: failed to create workspace: {}", e))
                .await?;
            return Ok(());
        }
        new_dir.display().to_string()
    } else {
        // Expand ~ to home directory
        let expanded = if path_str.starts_with("~/") || path_str == "~" {
            if let Some(home) = dirs::home_dir() {
                home.join(path_str.strip_prefix("~/").unwrap_or("")).display().to_string()
            } else {
                path_str.to_string()
            }
        } else {
            path_str.to_string()
        };
        // Validate path exists
        let path = Path::new(&expanded);
        if !path.exists() || !path.is_dir() {
            shared_rate_limit_wait(state, chat_id).await;
            bot.send_message(chat_id, format!("Error: '{}' is not a valid directory.", expanded))
                .await?;
            return Ok(());
        }
        path.canonicalize()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| expanded)
    };

    // Try to load existing session for this path
    let existing = bot_common::load_existing_session(&canonical_path);

    let mut response_lines = Vec::new();

    {
        let mut data = state.lock().await;
        let session = data.sessions.entry(chat_id).or_insert_with(|| ChatSession {
            session_id: None,
            current_path: None,
            history: Vec::new(),
            pending_uploads: Vec::new(),
            cleared: false,
        });

        if let Some((session_data, _)) = &existing {
            session.session_id = Some(session_data.session_id.clone());
            session.current_path = Some(canonical_path.clone());
            session.history = session_data.history.clone();

            let ts = chrono::Local::now().format("%H:%M:%S");
            println!("  [{ts}] ▶ Session restored: {canonical_path}");
            response_lines.push(format!("Session restored at `{}`.", canonical_path));
            response_lines.push(String::new());

            // Show last 5 conversation items
            let history_len = session_data.history.len();
            let start_idx = if history_len > 5 { history_len - 5 } else { 0 };
            for item in &session_data.history[start_idx..] {
                let prefix = match item.item_type {
                    HistoryType::User => "You",
                    HistoryType::Assistant => "AI",
                    HistoryType::Error => "Error",
                    HistoryType::System => "System",
                    HistoryType::ToolUse => "Tool",
                    HistoryType::ToolResult => "Result",
                };
                // Truncate long items for display
                let content: String = item.content.chars().take(200).collect();
                let truncated = if item.content.chars().count() > 200 { "..." } else { "" };
                response_lines.push(format!("[{}] {}{}", prefix, content, truncated));
            }
        } else {
            session.session_id = None;
            session.current_path = Some(canonical_path.clone());
            session.history.clear();

            let ts = chrono::Local::now().format("%H:%M:%S");
            println!("  [{ts}] ▶ Session started: {canonical_path}");
            response_lines.push(format!("Session started at `{}`.", canonical_path));
        }
    }

    // Persist chat_id → path mapping for auto-restore after restart
    {
        let mut data = state.lock().await;
        data.settings.last_sessions.insert(chat_id.0.to_string(), canonical_path);
        bot_common::save_bot_settings(&token_hash(token), &data.settings, &[("token", token)]);
    }

    let response_text = response_lines.join("\n");
    send_long_message(bot, chat_id, &response_text, None, state).await?;

    Ok(())
}

/// Handle /clear command
pub async fn handle_clear_command(
    bot: &Bot,
    chat_id: ChatId,
    state: &SharedState,
) -> ResponseResult<()> {
    // Cancel in-progress AI request if any
    let cancel_token = {
        let data = state.lock().await;
        data.cancel_tokens.get(&chat_id).cloned()
    };
    if let Some(token) = cancel_token {
        token.cancelled.store(true, Ordering::Relaxed);
        if let Ok(guard) = token.child_pid.lock() {
            if let Some(pid) = *guard {
                #[cfg(unix)]
                #[allow(unsafe_code)]
                unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM); }
            }
        }
    }

    {
        let mut data = state.lock().await;
        if let Some(session) = data.sessions.get_mut(&chat_id) {
            session.session_id = None;
            session.history.clear();
            session.pending_uploads.clear();
            session.cleared = true;
        }
        data.cancel_tokens.remove(&chat_id);
        data.stop_message_ids.remove(&chat_id);
    }

    shared_rate_limit_wait(state, chat_id).await;
    bot.send_message(chat_id, "Session cleared.")
        .await?;

    Ok(())
}

/// Handle /pwd command - show current session path
pub async fn handle_pwd_command(
    bot: &Bot,
    chat_id: ChatId,
    state: &SharedState,
) -> ResponseResult<()> {
    let current_path = {
        let data = state.lock().await;
        data.sessions.get(&chat_id).and_then(|s| s.current_path.clone())
    };

    shared_rate_limit_wait(state, chat_id).await;
    match current_path {
        Some(path) => bot.send_message(chat_id, &path).await?,
        None => bot.send_message(chat_id, "No active session. Use /start <path> first.").await?,
    };

    Ok(())
}

/// Handle /stop command - cancel in-progress AI request
pub async fn handle_stop_command(
    bot: &Bot,
    chat_id: ChatId,
    state: &SharedState,
) -> ResponseResult<()> {
    let token = {
        let data = state.lock().await;
        data.cancel_tokens.get(&chat_id).cloned()
    };

    match token {
        Some(token) => {
            // Ignore duplicate /stop if already cancelled
            if token.cancelled.load(Ordering::Relaxed) {
                return Ok(());
            }

            // Send immediate feedback to user
            shared_rate_limit_wait(state, chat_id).await;
            let stop_msg = bot.send_message(chat_id, "Stopping...").await?;

            // Store the stop message ID so the polling loop can update it later
            {
                let mut data = state.lock().await;
                data.stop_message_ids.insert(chat_id, stop_msg.id);
            }

            // Set cancellation flag
            token.cancelled.store(true, Ordering::Relaxed);

            // Kill child process directly to unblock reader.lines()
            // When the child dies, its stdout pipe closes → reader returns EOF → blocking thread exits
            if let Ok(guard) = token.child_pid.lock() {
                if let Some(pid) = *guard {
                    #[cfg(unix)]
                    #[allow(unsafe_code)]
                    unsafe {
                        libc::kill(pid as libc::pid_t, libc::SIGTERM);
                    }
                }
            }

            let ts = chrono::Local::now().format("%H:%M:%S");
            println!("  [{ts}] ■ Cancel signal sent");
        }
        None => {
            shared_rate_limit_wait(state, chat_id).await;
            bot.send_message(chat_id, "No active request to stop.")
                .await?;
        }
    }

    Ok(())
}

/// Handle /down <filepath> - send file to user
pub async fn handle_down_command(
    bot: &Bot,
    chat_id: ChatId,
    text: &str,
    state: &SharedState,
) -> ResponseResult<()> {
    let file_path = text.strip_prefix("/down").unwrap_or("").trim();

    if file_path.is_empty() {
        shared_rate_limit_wait(state, chat_id).await;
        bot.send_message(chat_id, "Usage: /down <filepath>\nExample: /down /home/kst/file.txt")
            .await?;
        return Ok(());
    }

    // Resolve relative path using current session path
    let resolved_path = if Path::new(file_path).is_absolute() {
        file_path.to_string()
    } else {
        let current_path = {
            let data = state.lock().await;
            data.sessions.get(&chat_id).and_then(|s| s.current_path.clone())
        };
        match current_path {
            Some(base) => format!("{}/{}", base.trim_end_matches('/'), file_path),
            None => {
                shared_rate_limit_wait(state, chat_id).await;
                bot.send_message(chat_id, "No active session. Use absolute path or /start <path> first.")
                    .await?;
                return Ok(());
            }
        }
    };

    let path = Path::new(&resolved_path);
    if !path.exists() {
        shared_rate_limit_wait(state, chat_id).await;
        bot.send_message(chat_id, &format!("File not found: {}", resolved_path)).await?;
        return Ok(());
    }
    if !path.is_file() {
        shared_rate_limit_wait(state, chat_id).await;
        bot.send_message(chat_id, &format!("Not a file: {}", resolved_path)).await?;
        return Ok(());
    }

    shared_rate_limit_wait(state, chat_id).await;
    bot.send_document(chat_id, teloxide::types::InputFile::file(path))
        .await?;

    Ok(())
}

/// Handle file/photo upload - save to current session path
pub async fn handle_file_upload(
    bot: &Bot,
    chat_id: ChatId,
    msg: &teloxide::types::Message,
    state: &SharedState,
) -> ResponseResult<()> {
    // Get current session path
    let current_path = {
        let data = state.lock().await;
        data.sessions.get(&chat_id).and_then(|s| s.current_path.clone())
    };

    let Some(save_dir) = current_path else {
        shared_rate_limit_wait(state, chat_id).await;
        bot.send_message(chat_id, "No active session. Use /start <path> first.")
            .await?;
        return Ok(());
    };

    // Get file_id and file_name
    let (file_id, file_name) = if let Some(doc) = msg.document() {
        let name = doc.file_name.clone().unwrap_or_else(|| "uploaded_file".to_string());
        (doc.file.id.clone(), name)
    } else if let Some(photos) = msg.photo() {
        // Get the largest photo
        if let Some(photo) = photos.last() {
            let name = format!("photo_{}.jpg", photo.file.unique_id);
            (photo.file.id.clone(), name)
        } else {
            return Ok(());
        }
    } else {
        return Ok(());
    };

    // Download file from Telegram via HTTP
    shared_rate_limit_wait(state, chat_id).await;
    let file = bot.get_file(&file_id).await?;
    let url = format!("https://api.telegram.org/file/bot{}/{}", bot.token(), file.path);
    let buf = match reqwest::get(&url).await {
        Ok(resp) => match resp.bytes().await {
            Ok(bytes) => bytes,
            Err(e) => {
                shared_rate_limit_wait(state, chat_id).await;
                bot.send_message(chat_id, &format!("Download failed: {}", e)).await?;
                return Ok(());
            }
        },
        Err(e) => {
            shared_rate_limit_wait(state, chat_id).await;
            bot.send_message(chat_id, &format!("Download failed: {}", e)).await?;
            return Ok(());
        }
    };

    // Save to session path (sanitize file_name to prevent path traversal)
    let safe_name = Path::new(&file_name)
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("uploaded_file"));
    let dest = Path::new(&save_dir).join(safe_name);
    let file_size = buf.len();
    match fs::write(&dest, &buf) {
        Ok(_) => {
            let msg_text = format!("Saved: {}\n({} bytes)", dest.display(), file_size);
            shared_rate_limit_wait(state, chat_id).await;
            bot.send_message(chat_id, &msg_text).await?;
        }
        Err(e) => {
            shared_rate_limit_wait(state, chat_id).await;
            bot.send_message(chat_id, &format!("Failed to save file: {}", e)).await?;
            return Ok(());
        }
    }

    // Record upload in session history and pending queue for Claude
    let upload_record = format!(
        "[File uploaded] {} → {} ({} bytes)",
        file_name, dest.display(), file_size
    );
    {
        let mut data = state.lock().await;
        if let Some(session) = data.sessions.get_mut(&chat_id) {
            session.history.push(HistoryItem {
                item_type: HistoryType::User,
                content: upload_record.clone(),
            });
            session.pending_uploads.push(upload_record);
            bot_common::save_session_to_file(session.session_id.as_deref(), &session.history, &save_dir);
        }
    }

    Ok(())
}

/// Handle !command - execute shell command directly
pub async fn handle_shell_command(
    bot: &Bot,
    chat_id: ChatId,
    text: &str,
    state: &SharedState,
) -> ResponseResult<()> {
    let cmd_str = text.strip_prefix('!').unwrap_or("").trim();

    if cmd_str.is_empty() {
        shared_rate_limit_wait(state, chat_id).await;
        bot.send_message(chat_id, "Usage: !<command>\nExample: !mkdir /home/kst/testcode")
            .await?;
        return Ok(());
    }

    // Get current_path for working directory (default to home directory)
    let working_dir = {
        let data = state.lock().await;
        data.sessions.get(&chat_id)
            .and_then(|s| s.current_path.clone())
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .map(|h| h.display().to_string())
                    .unwrap_or_else(|| "/".to_string())
            })
    };

    let cmd_owned = cmd_str.to_string();
    let working_dir_clone = working_dir.clone();

    // Run shell command in blocking thread with stdin closed and timeout
    let result = tokio::task::spawn_blocking(move || {
        let child = std::process::Command::new("bash")
            .args(["-c", &cmd_owned])
            .current_dir(&working_dir_clone)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();

        match child {
            Ok(child) => child.wait_with_output(),
            Err(e) => Err(e),
        }
    }).await;

    let response = match result {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let exit_code = output.status.code().unwrap_or(-1);

            let mut parts = Vec::new();

            if !stdout.is_empty() {
                let trimmed = stdout.trim_end();
                parts.push(format!("<pre>{}</pre>", html_escape(trimmed)));
            }
            if !stderr.is_empty() {
                parts.push(format!("stderr:\n<pre>{}</pre>", html_escape(stderr.trim_end())));
            }
            if parts.is_empty() {
                parts.push(format!("(exit code: {})", exit_code));
            } else if exit_code != 0 {
                parts.push(format!("(exit code: {})", exit_code));
            }

            parts.join("\n")
        }
        Ok(Err(e)) => format!("Failed to execute: {}", html_escape(&e.to_string())),
        Err(e) => format!("Task error: {}", html_escape(&e.to_string())),
    };

    send_long_message(bot, chat_id, &response, Some(ParseMode::Html), state).await?;

    Ok(())
}

/// Handle /availabletools command - show all available tools
pub async fn handle_availabletools_command(
    bot: &Bot,
    chat_id: ChatId,
    state: &SharedState,
) -> ResponseResult<()> {
    let mut msg = String::from("<b>Available Tools</b>\n\n");

    for &(name, desc, destructive) in ALL_TOOLS {
        let badge = risk_badge(destructive);
        if badge.is_empty() {
            msg.push_str(&format!("<code>{}</code> — {}\n", html_escape(name), html_escape(desc)));
        } else {
            msg.push_str(&format!("<code>{}</code> {} — {}\n", html_escape(name), badge, html_escape(desc)));
        }
    }
    msg.push_str(&format!("\n{} = destructive\nTotal: {}", risk_badge(true), ALL_TOOLS.len()));

    send_long_message(bot, chat_id, &msg, Some(ParseMode::Html), state).await?;

    Ok(())
}

/// Handle /allowedtools command - show current allowed tools list
pub async fn handle_allowedtools_command(
    bot: &Bot,
    chat_id: ChatId,
    state: &SharedState,
) -> ResponseResult<()> {
    let tools = {
        let data = state.lock().await;
        data.settings.allowed_tools.clone()
    };

    let mut msg = String::from("<b>Allowed Tools</b>\n\n");
    for tool in &tools {
        let (desc, destructive) = tool_info(tool);
        let badge = risk_badge(destructive);
        if badge.is_empty() {
            msg.push_str(&format!("<code>{}</code> — {}\n", html_escape(tool), html_escape(desc)));
        } else {
            msg.push_str(&format!("<code>{}</code> {} — {}\n", html_escape(tool), badge, html_escape(desc)));
        }
    }
    msg.push_str(&format!("\n{} = destructive\nTotal: {}", risk_badge(true), tools.len()));

    shared_rate_limit_wait(state, chat_id).await;
    bot.send_message(chat_id, &msg)
        .parse_mode(ParseMode::Html)
        .await?;

    Ok(())
}

/// Handle /allowed command - add/remove tools
/// Usage: /allowed +toolname  (add)
///        /allowed -toolname  (remove)
pub async fn handle_allowed_command(
    bot: &Bot,
    chat_id: ChatId,
    text: &str,
    state: &SharedState,
    token: &str,
) -> ResponseResult<()> {
    let arg = text.strip_prefix("/allowed").unwrap_or("").trim();

    if arg.is_empty() {
        shared_rate_limit_wait(state, chat_id).await;
        bot.send_message(chat_id, "Usage:\n/allowed +toolname — Add a tool\n/allowed -toolname — Remove a tool\n/allowedtools — Show current list")
            .await?;
        return Ok(());
    }

    // Skip if argument starts with "tools" (that's /allowedtools handled separately)
    if arg.starts_with("tools") {
        // This shouldn't happen due to routing order, but just in case
        return handle_allowedtools_command(bot, chat_id, state).await;
    }

    let (op, raw_name) = if let Some(name) = arg.strip_prefix('+') {
        ('+', name.trim())
    } else if let Some(name) = arg.strip_prefix('-') {
        ('-', name.trim())
    } else {
        shared_rate_limit_wait(state, chat_id).await;
        bot.send_message(chat_id, "Use +toolname to add or -toolname to remove.\nExample: /allowed +Bash")
            .await?;
        return Ok(());
    };

    if raw_name.is_empty() {
        shared_rate_limit_wait(state, chat_id).await;
        bot.send_message(chat_id, "Tool name cannot be empty.")
            .await?;
        return Ok(());
    }

    let tool_name = normalize_tool_name(raw_name);

    let response_msg = {
        let mut data = state.lock().await;
        match op {
            '+' => {
                if data.settings.allowed_tools.iter().any(|t| t == &tool_name) {
                    format!("<code>{}</code> is already in the list.", html_escape(&tool_name))
                } else {
                    data.settings.allowed_tools.push(tool_name.clone());
                    bot_common::save_bot_settings(&token_hash(token), &data.settings, &[("token", token)]);
                    format!("✅ Added <code>{}</code>", html_escape(&tool_name))
                }
            }
            '-' => {
                let before_len = data.settings.allowed_tools.len();
                data.settings.allowed_tools.retain(|t| t != &tool_name);
                if data.settings.allowed_tools.len() < before_len {
                    bot_common::save_bot_settings(&token_hash(token), &data.settings, &[("token", token)]);
                    format!("❌ Removed <code>{}</code>", html_escape(&tool_name))
                } else {
                    format!("<code>{}</code> is not in the list.", html_escape(&tool_name))
                }
            }
            _ => unreachable!(),
        }
    };

    shared_rate_limit_wait(state, chat_id).await;
    bot.send_message(chat_id, &response_msg)
        .parse_mode(ParseMode::Html)
        .await?;

    Ok(())
}

/// Handle /resume command - list saved sessions or resume a specific one
/// Usage: /resume              (list all saved sessions)
///        /resume <number>     (resume session by number from the list)
pub async fn handle_resume_command(
    bot: &Bot,
    chat_id: ChatId,
    text: &str,
    state: &SharedState,
    token: &str,
) -> ResponseResult<()> {
    let arg = text.strip_prefix("/resume").unwrap_or("").trim();

    let sessions = bot_common::list_all_sessions();

    if sessions.is_empty() {
        shared_rate_limit_wait(state, chat_id).await;
        bot.send_message(chat_id, "No saved sessions found.")
            .await?;
        return Ok(());
    }

    if arg.is_empty() {
        // List all saved sessions
        let mut msg = String::from("<b>Saved Sessions</b>\n\n");
        let max_display = 10;
        for (i, s) in sessions.iter().take(max_display).enumerate() {
            // Shorten path for display
            let path_display: String = s.current_path.chars().take(40).collect();
            let path_suffix = if s.current_path.chars().count() > 40 { "..." } else { "" };
            msg.push_str(&format!(
                "<code>{}</code>. <code>{}{}</code>\n    {} | {} msgs\n",
                i + 1,
                html_escape(&path_display),
                path_suffix,
                html_escape(&s.created_at),
                s.history_count,
            ));
        }
        if sessions.len() > max_display {
            msg.push_str(&format!("\n... and {} more", sessions.len() - max_display));
        }
        msg.push_str("\nResume: <code>/resume &lt;number&gt;</code>");

        shared_rate_limit_wait(state, chat_id).await;
        bot.send_message(chat_id, &msg)
            .parse_mode(ParseMode::Html)
            .await?;
        return Ok(());
    }

    // Parse number argument
    let num: usize = match arg.parse() {
        Ok(n) if n >= 1 && n <= sessions.len() => n,
        _ => {
            shared_rate_limit_wait(state, chat_id).await;
            bot.send_message(chat_id, &format!("Invalid number. Use 1-{}.", sessions.len()))
                .await?;
            return Ok(());
        }
    };

    let selected = &sessions[num - 1];

    // Validate the path still exists
    if !std::path::Path::new(&selected.current_path).is_dir() {
        shared_rate_limit_wait(state, chat_id).await;
        bot.send_message(chat_id, &format!(
            "Directory no longer exists: {}",
            html_escape(&selected.current_path)
        ))
            .parse_mode(ParseMode::Html)
            .await?;
        return Ok(());
    }

    // Load full session data
    let session_data = match bot_common::load_session_by_id(&selected.session_id) {
        Some(data) => data,
        None => {
            shared_rate_limit_wait(state, chat_id).await;
            bot.send_message(chat_id, "Failed to load session data.").await?;
            return Ok(());
        }
    };

    let canonical_path = selected.current_path.clone();
    let mut response_lines = Vec::new();

    {
        let mut data = state.lock().await;
        let session = data.sessions.entry(chat_id).or_insert_with(|| ChatSession {
            session_id: None,
            current_path: None,
            history: Vec::new(),
            pending_uploads: Vec::new(),
            cleared: false,
        });

        session.session_id = Some(session_data.session_id.clone());
        session.current_path = Some(canonical_path.clone());
        session.history = session_data.history.clone();
        session.pending_uploads.clear();
        session.cleared = false;

        let ts = chrono::Local::now().format("%H:%M:%S");
        println!("  [{ts}] ▶ Session resumed: {canonical_path}");
        response_lines.push(format!("Session resumed at `{}`.", canonical_path));
        response_lines.push(String::new());

        // Show last 5 conversation items
        let history_len = session_data.history.len();
        let start_idx = if history_len > 5 { history_len - 5 } else { 0 };
        for item in &session_data.history[start_idx..] {
            let prefix = match item.item_type {
                HistoryType::User => "You",
                HistoryType::Assistant => "AI",
                HistoryType::Error => "Error",
                HistoryType::System => "System",
                HistoryType::ToolUse => "Tool",
                HistoryType::ToolResult => "Result",
            };
            let content: String = item.content.chars().take(200).collect();
            let truncated = if item.content.chars().count() > 200 { "..." } else { "" };
            response_lines.push(format!("[{}] {}{}", prefix, content, truncated));
        }
    }

    // Persist chat_id → path mapping for auto-restore
    {
        let mut data = state.lock().await;
        data.settings.last_sessions.insert(chat_id.0.to_string(), canonical_path);
        bot_common::save_bot_settings(&token_hash(token), &data.settings, &[("token", token)]);
    }

    let response_text = response_lines.join("\n");
    send_long_message(bot, chat_id, &response_text, None, state).await?;

    Ok(())
}

/// Handle /agent command - switch AI agent or show current agent
/// Usage: /agent               (show current agent)
///        /agent <name>        (switch to agent)
pub async fn handle_agent_command(
    bot: &Bot,
    chat_id: ChatId,
    text: &str,
    state: &SharedState,
) -> ResponseResult<()> {
    let arg = text.strip_prefix("/agent").unwrap_or("").trim();

    if arg.is_empty() {
        // Show current agent and list of available agents
        let current = {
            let data = state.lock().await;
            data.agent_type.clone()
        };

        let mut msg = format!("<b>Current agent:</b> <code>{}</code>\n\n<b>Available agents:</b>\n", html_escape(&current));
        for &(name, desc) in AVAILABLE_AGENTS {
            let marker = if name == current { " ◀" } else { "" };
            msg.push_str(&format!("<code>{}</code> — {}{}\n", html_escape(name), html_escape(desc), marker));
        }
        msg.push_str("\nSwitch: <code>/agent &lt;name&gt;</code>");

        shared_rate_limit_wait(state, chat_id).await;
        bot.send_message(chat_id, &msg)
            .parse_mode(ParseMode::Html)
            .await?;
        return Ok(());
    }

    let agent_name = arg.to_lowercase();

    if !is_valid_agent(&agent_name) {
        let valid: Vec<&str> = AVAILABLE_AGENTS.iter().map(|(n, _)| *n).collect();
        shared_rate_limit_wait(state, chat_id).await;
        bot.send_message(chat_id, &format!(
            "Unknown agent: <code>{}</code>\nAvailable: {}",
            html_escape(&agent_name),
            valid.iter().map(|n| format!("<code>{}</code>", n)).collect::<Vec<_>>().join(", ")
        ))
            .parse_mode(ParseMode::Html)
            .await?;
        return Ok(());
    }

    // Switch agent
    let old_agent = {
        let mut data = state.lock().await;
        let old = data.agent_type.clone();
        data.agent_type = agent_name.clone();
        old
    };

    let response = if old_agent == agent_name {
        format!("Already using <code>{}</code>.", html_escape(&agent_name))
    } else {
        format!("Switched: <code>{}</code> → <code>{}</code>", html_escape(&old_agent), html_escape(&agent_name))
    };

    shared_rate_limit_wait(state, chat_id).await;
    bot.send_message(chat_id, &response)
        .parse_mode(ParseMode::Html)
        .await?;

    Ok(())
}
