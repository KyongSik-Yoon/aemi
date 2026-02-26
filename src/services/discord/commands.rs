use std::sync::atomic::Ordering;
use std::path::Path;
use std::fs;

use serenity::builder::{CreateAttachment, CreateMessage};
use serenity::model::channel::Message;
use serenity::model::id::ChannelId;
use serenity::prelude::*;

use crate::services::session::{HistoryItem, HistoryType};
use crate::services::bot_common::{self, ALL_TOOLS, AVAILABLE_AGENTS, normalize_tool_name, tool_info, risk_badge, is_valid_agent};
use crate::services::formatter;

use super::{ChannelSession, SharedState, discord_token_hash};
use super::messages::{rate_limit_wait, send_long_message};

/// Handle /help command
pub async fn handle_help_command(
    ctx: &Context,
    channel_id: ChannelId,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let help = "\
**aemi Discord Bot**
Manage server files & chat with Claude AI.

**Session**
`/start <path>` — Start session at directory
`/start` — Start with auto-generated workspace
`/pwd` — Show current working directory
`/clear` — Clear AI conversation history
`/stop` — Stop current AI request

**File Transfer**
`/down <file>` — Download file from server
Send a file — Upload to session directory

**Shell**
`!<command>` — Run shell command directly
  e.g. `!ls -la`, `!git status`

**AI Chat**
Any other message is sent to AI agent.
AI can read, edit, and run commands in your session.

**Agent**
`/agent` — Show current AI agent
`/agent <name>` — Switch agent (claude, gemini, codex, opencode)

**Tool Management**
`/availabletools` — List all available tools
`/allowedtools` — Show currently allowed tools
`/allowed +name` — Add tool (e.g. `/allowed +Bash`)
`/allowed -name` — Remove tool

`/help` — Show this help";

    rate_limit_wait(state, channel_id).await;
    channel_id.say(&ctx.http, help).await?;

    Ok(())
}

/// Handle /start <path> command
pub async fn handle_start_command(
    ctx: &Context,
    channel_id: ChannelId,
    text: &str,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let path_str = text.strip_prefix("/start").unwrap_or("").trim();

    let canonical_path = if path_str.is_empty() {
        let Some(home) = dirs::home_dir() else {
            rate_limit_wait(state, channel_id).await;
            channel_id.say(&ctx.http, "Error: cannot determine home directory.").await?;
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
            rate_limit_wait(state, channel_id).await;
            channel_id.say(&ctx.http, &format!("Error: failed to create workspace: {}", e)).await?;
            return Ok(());
        }
        new_dir.display().to_string()
    } else {
        let expanded = if path_str.starts_with("~/") || path_str == "~" {
            if let Some(home) = dirs::home_dir() {
                home.join(path_str.strip_prefix("~/").unwrap_or("")).display().to_string()
            } else {
                path_str.to_string()
            }
        } else {
            path_str.to_string()
        };
        let path = Path::new(&expanded);
        if !path.exists() || !path.is_dir() {
            rate_limit_wait(state, channel_id).await;
            channel_id.say(&ctx.http, &format!("Error: '{}' is not a valid directory.", expanded)).await?;
            return Ok(());
        }
        path.canonicalize()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| expanded)
    };

    let existing = bot_common::load_existing_session(&canonical_path);

    let mut response_lines = Vec::new();

    let token = {
        let mut data = state.lock().await;
        let session = data.sessions.entry(channel_id).or_insert_with(|| ChannelSession {
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
        } else {
            session.session_id = None;
            session.current_path = Some(canonical_path.clone());
            session.history.clear();

            let ts = chrono::Local::now().format("%H:%M:%S");
            println!("  [{ts}] ▶ Session started: {canonical_path}");
            response_lines.push(format!("Session started at `{}`.", canonical_path));
        }

        data.token.clone()
    };

    // Persist channel_id → path mapping for auto-restore
    {
        let mut data = state.lock().await;
        data.settings.last_sessions.insert(channel_id.get().to_string(), canonical_path);
        bot_common::save_bot_settings(&discord_token_hash(&token), &data.settings, &[("platform", "discord")]);
    }

    let response_text = response_lines.join("\n");
    send_long_message(ctx, channel_id, &response_text, state).await?;

    Ok(())
}

/// Handle /clear command
pub async fn handle_clear_command(
    ctx: &Context,
    channel_id: ChannelId,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Cancel in-progress AI request if any
    let cancel_token = {
        let data = state.lock().await;
        data.cancel_tokens.get(&channel_id).cloned()
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
        if let Some(session) = data.sessions.get_mut(&channel_id) {
            session.session_id = None;
            session.history.clear();
            session.pending_uploads.clear();
            session.cleared = true;
        }
        data.cancel_tokens.remove(&channel_id);
    }

    rate_limit_wait(state, channel_id).await;
    channel_id.say(&ctx.http, "Session cleared.").await?;

    Ok(())
}

/// Handle /pwd command
pub async fn handle_pwd_command(
    ctx: &Context,
    channel_id: ChannelId,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let current_path = {
        let data = state.lock().await;
        data.sessions.get(&channel_id).and_then(|s| s.current_path.clone())
    };

    rate_limit_wait(state, channel_id).await;
    match current_path {
        Some(path) => channel_id.say(&ctx.http, &path).await?,
        None => channel_id.say(&ctx.http, "No active session. Use /start <path> first.").await?,
    };

    Ok(())
}

/// Handle /stop command
pub async fn handle_stop_command(
    ctx: &Context,
    channel_id: ChannelId,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let token = {
        let data = state.lock().await;
        data.cancel_tokens.get(&channel_id).cloned()
    };

    match token {
        Some(token) => {
            if token.cancelled.load(Ordering::Relaxed) {
                return Ok(());
            }

            rate_limit_wait(state, channel_id).await;
            channel_id.say(&ctx.http, "Stopping...").await?;

            token.cancelled.store(true, Ordering::Relaxed);

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
            rate_limit_wait(state, channel_id).await;
            channel_id.say(&ctx.http, "No active request to stop.").await?;
        }
    }

    Ok(())
}

/// Handle /down <filepath> - send file to user
pub async fn handle_down_command(
    ctx: &Context,
    channel_id: ChannelId,
    text: &str,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let file_path = text.strip_prefix("/down").unwrap_or("").trim();

    if file_path.is_empty() {
        rate_limit_wait(state, channel_id).await;
        channel_id.say(&ctx.http, "Usage: /down <filepath>\nExample: /down /home/kst/file.txt").await?;
        return Ok(());
    }

    let resolved_path = if Path::new(file_path).is_absolute() {
        file_path.to_string()
    } else {
        let current_path = {
            let data = state.lock().await;
            data.sessions.get(&channel_id).and_then(|s| s.current_path.clone())
        };
        match current_path {
            Some(base) => format!("{}/{}", base.trim_end_matches('/'), file_path),
            None => {
                rate_limit_wait(state, channel_id).await;
                channel_id.say(&ctx.http, "No active session. Use absolute path or /start <path> first.").await?;
                return Ok(());
            }
        }
    };

    let path = Path::new(&resolved_path);
    if !path.exists() {
        rate_limit_wait(state, channel_id).await;
        channel_id.say(&ctx.http, &format!("File not found: {}", resolved_path)).await?;
        return Ok(());
    }
    if !path.is_file() {
        rate_limit_wait(state, channel_id).await;
        channel_id.say(&ctx.http, &format!("Not a file: {}", resolved_path)).await?;
        return Ok(());
    }

    // Read file and send as attachment
    rate_limit_wait(state, channel_id).await;
    let attachment = CreateAttachment::path(path).await?;
    let builder = CreateMessage::new();
    channel_id.send_files(&ctx.http, vec![attachment], builder).await?;

    Ok(())
}

/// Handle file attachment upload - save to current session path
pub async fn handle_file_upload(
    ctx: &Context,
    msg: &Message,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let channel_id = msg.channel_id;

    let current_path = {
        let data = state.lock().await;
        data.sessions.get(&channel_id).and_then(|s| s.current_path.clone())
    };

    let Some(save_dir) = current_path else {
        rate_limit_wait(state, channel_id).await;
        channel_id.say(&ctx.http, "No active session. Use /start <path> first.").await?;
        return Ok(());
    };

    for attachment in &msg.attachments {
        let file_name = &attachment.filename;
        let url = &attachment.url;

        // Download file via HTTP
        let buf = match reqwest::get(url).await {
            Ok(resp) => match resp.bytes().await {
                Ok(bytes) => bytes,
                Err(e) => {
                    rate_limit_wait(state, channel_id).await;
                    channel_id.say(&ctx.http, &format!("Download failed: {}", e)).await?;
                    continue;
                }
            },
            Err(e) => {
                rate_limit_wait(state, channel_id).await;
                channel_id.say(&ctx.http, &format!("Download failed: {}", e)).await?;
                continue;
            }
        };

        // Save to session path (sanitize file_name to prevent path traversal)
        let safe_name = Path::new(file_name)
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("uploaded_file"));
        let dest = Path::new(&save_dir).join(safe_name);
        let file_size = buf.len();
        match fs::write(&dest, &buf) {
            Ok(_) => {
                let msg_text = format!("Saved: {}\n({} bytes)", dest.display(), file_size);
                rate_limit_wait(state, channel_id).await;
                channel_id.say(&ctx.http, &msg_text).await?;
            }
            Err(e) => {
                rate_limit_wait(state, channel_id).await;
                channel_id.say(&ctx.http, &format!("Failed to save file: {}", e)).await?;
                continue;
            }
        }

        // Record upload in session history
        let upload_record = format!(
            "[File uploaded] {} → {} ({} bytes)",
            file_name, dest.display(), file_size
        );
        {
            let mut data = state.lock().await;
            if let Some(session) = data.sessions.get_mut(&channel_id) {
                session.history.push(HistoryItem {
                    item_type: HistoryType::User,
                    content: upload_record.clone(),
                });
                session.pending_uploads.push(upload_record);
                bot_common::save_session_to_file(session.session_id.as_deref(), &session.history, &save_dir);
            }
        }
    }

    Ok(())
}

/// Handle !command - execute shell command directly
pub async fn handle_shell_command(
    ctx: &Context,
    channel_id: ChannelId,
    text: &str,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cmd_str = text.strip_prefix('!').unwrap_or("").trim();

    if cmd_str.is_empty() {
        rate_limit_wait(state, channel_id).await;
        channel_id.say(&ctx.http, "Usage: !<command>\nExample: !mkdir /home/kst/testcode").await?;
        return Ok(());
    }

    let working_dir = {
        let data = state.lock().await;
        data.sessions.get(&channel_id)
            .and_then(|s| s.current_path.clone())
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .map(|h| h.display().to_string())
                    .unwrap_or_else(|| "/".to_string())
            })
    };

    let cmd_owned = cmd_str.to_string();
    let working_dir_clone = working_dir.clone();

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
                let clean = formatter::strip_ansi_codes(stdout.trim_end());
                if formatter::is_diff_content(&clean) {
                    parts.push(format!("```diff\n{}\n```", clean));
                } else {
                    parts.push(format!("```\n{}\n```", clean));
                }
            }
            if !stderr.is_empty() {
                let clean_err = formatter::strip_ansi_codes(stderr.trim_end());
                parts.push(format!("stderr:\n```\n{}\n```", clean_err));
            }
            if parts.is_empty() {
                parts.push(format!("(exit code: {})", exit_code));
            } else if exit_code != 0 {
                parts.push(format!("(exit code: {})", exit_code));
            }

            parts.join("\n")
        }
        Ok(Err(e)) => format!("Failed to execute: {}", e),
        Err(e) => format!("Task error: {}", e),
    };

    send_long_message(ctx, channel_id, &response, state).await?;

    Ok(())
}

/// Handle /availabletools command
pub async fn handle_availabletools_command(
    ctx: &Context,
    channel_id: ChannelId,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut msg = String::from("**Available Tools**\n\n");

    for &(name, desc, destructive) in ALL_TOOLS {
        let badge = risk_badge(destructive);
        if badge.is_empty() {
            msg.push_str(&format!("`{}` — {}\n", name, desc));
        } else {
            msg.push_str(&format!("`{}` {} — {}\n", name, badge, desc));
        }
    }
    msg.push_str(&format!("\n{} = destructive\nTotal: {}", risk_badge(true), ALL_TOOLS.len()));

    send_long_message(ctx, channel_id, &msg, state).await?;

    Ok(())
}

/// Handle /allowedtools command
pub async fn handle_allowedtools_command(
    ctx: &Context,
    channel_id: ChannelId,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let tools = {
        let data = state.lock().await;
        data.settings.allowed_tools.clone()
    };

    let mut msg = String::from("**Allowed Tools**\n\n");
    for tool in &tools {
        let (desc, destructive) = tool_info(tool);
        let badge = risk_badge(destructive);
        if badge.is_empty() {
            msg.push_str(&format!("`{}` — {}\n", tool, desc));
        } else {
            msg.push_str(&format!("`{}` {} — {}\n", tool, badge, desc));
        }
    }
    msg.push_str(&format!("\n{} = destructive\nTotal: {}", risk_badge(true), tools.len()));

    send_long_message(ctx, channel_id, &msg, state).await?;

    Ok(())
}

/// Handle /allowed command - add/remove tools
pub async fn handle_allowed_command(
    ctx: &Context,
    channel_id: ChannelId,
    text: &str,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let arg = text.strip_prefix("/allowed").unwrap_or("").trim();

    if arg.is_empty() {
        rate_limit_wait(state, channel_id).await;
        channel_id.say(&ctx.http, "Usage:\n/allowed +toolname — Add a tool\n/allowed -toolname — Remove a tool\n/allowedtools — Show current list").await?;
        return Ok(());
    }

    if arg.starts_with("tools") {
        return handle_allowedtools_command(ctx, channel_id, state).await;
    }

    let (op, raw_name) = if let Some(name) = arg.strip_prefix('+') {
        ('+', name.trim())
    } else if let Some(name) = arg.strip_prefix('-') {
        ('-', name.trim())
    } else {
        rate_limit_wait(state, channel_id).await;
        channel_id.say(&ctx.http, "Use +toolname to add or -toolname to remove.\nExample: /allowed +Bash").await?;
        return Ok(());
    };

    if raw_name.is_empty() {
        rate_limit_wait(state, channel_id).await;
        channel_id.say(&ctx.http, "Tool name cannot be empty.").await?;
        return Ok(());
    }

    let tool_name = normalize_tool_name(raw_name);

    let response_msg = {
        let mut data = state.lock().await;
        let token = data.token.clone();
        match op {
            '+' => {
                if data.settings.allowed_tools.iter().any(|t| t == &tool_name) {
                    format!("`{}` is already in the list.", tool_name)
                } else {
                    data.settings.allowed_tools.push(tool_name.clone());
                    bot_common::save_bot_settings(&discord_token_hash(&token), &data.settings, &[("platform", "discord")]);
                    format!("Added `{}`", tool_name)
                }
            }
            '-' => {
                let before_len = data.settings.allowed_tools.len();
                data.settings.allowed_tools.retain(|t| t != &tool_name);
                if data.settings.allowed_tools.len() < before_len {
                    bot_common::save_bot_settings(&discord_token_hash(&token), &data.settings, &[("platform", "discord")]);
                    format!("Removed `{}`", tool_name)
                } else {
                    format!("`{}` is not in the list.", tool_name)
                }
            }
            _ => unreachable!(),
        }
    };

    rate_limit_wait(state, channel_id).await;
    channel_id.say(&ctx.http, &response_msg).await?;

    Ok(())
}

/// Handle /agent command - switch AI agent or show current agent
/// Usage: /agent               (show current agent)
///        /agent <name>        (switch to agent)
pub async fn handle_agent_command(
    ctx: &Context,
    channel_id: ChannelId,
    text: &str,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let arg = text.strip_prefix("/agent").unwrap_or("").trim();

    if arg.is_empty() {
        // Show current agent and list of available agents
        let current = {
            let data = state.lock().await;
            data.agent_type.clone()
        };

        let mut msg = format!("**Current agent:** `{}`\n\n**Available agents:**\n", current);
        for &(name, desc) in AVAILABLE_AGENTS {
            let marker = if name == current { " ◀" } else { "" };
            msg.push_str(&format!("`{}` — {}{}\n", name, desc, marker));
        }
        msg.push_str("\nSwitch: `/agent <name>`");

        rate_limit_wait(state, channel_id).await;
        channel_id.say(&ctx.http, &msg).await?;
        return Ok(());
    }

    let agent_name = arg.to_lowercase();

    if !is_valid_agent(&agent_name) {
        let valid: Vec<&str> = AVAILABLE_AGENTS.iter().map(|(n, _)| *n).collect();
        rate_limit_wait(state, channel_id).await;
        channel_id.say(&ctx.http, &format!(
            "Unknown agent: `{}`\nAvailable: {}",
            agent_name,
            valid.iter().map(|n| format!("`{}`", n)).collect::<Vec<_>>().join(", ")
        )).await?;
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
        format!("Already using `{}`.", agent_name)
    } else {
        format!("Switched: `{}` → `{}`", old_agent, agent_name)
    };

    rate_limit_wait(state, channel_id).await;
    channel_id.say(&ctx.http, &response).await?;

    Ok(())
}
