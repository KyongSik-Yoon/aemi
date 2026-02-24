use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::path::Path;
use std::fs;

use tokio::sync::Mutex;
use serenity::async_trait;
use serenity::builder::{CreateAttachment, CreateMessage, EditMessage};
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::id::ChannelId;
use serenity::prelude::*;
use sha2::{Sha256, Digest};

use crate::services::agent::{CancelToken, StreamMessage};
use crate::services::claude::{self, DEFAULT_ALLOWED_TOOLS};
use crate::services::gemini;
use crate::services::codex;
use crate::services::opencode;
use crate::services::session::{self, HistoryItem, HistoryType};
use crate::services::formatter;
use crate::services::utils::{floor_char_boundary, truncate_str, normalize_empty_lines};
use crate::services::bot_common::{self, BotSettings, ALL_TOOLS, normalize_tool_name, tool_info, risk_badge};

/// Per-channel session state
struct ChannelSession {
    session_id: Option<String>,
    current_path: Option<String>,
    history: Vec<HistoryItem>,
    /// File upload records not yet sent to Claude AI.
    pending_uploads: Vec<String>,
    /// Set to true by /clear to prevent a racing polling loop from re-populating history.
    cleared: bool,
}

/// Shared state: per-channel sessions + bot settings
struct SharedData {
    sessions: HashMap<ChannelId, ChannelSession>,
    settings: BotSettings,
    /// Per-channel cancel tokens for stopping in-progress AI requests
    cancel_tokens: HashMap<ChannelId, Arc<CancelToken>>,
    /// Per-channel timestamp of the last Discord API call (for rate limiting)
    api_timestamps: HashMap<ChannelId, tokio::time::Instant>,
    /// If set, only messages from this channel ID are allowed (--channel-id parameter)
    allowed_channel_id: Option<u64>,
    /// Bot token (stored for settings persistence)
    token: String,
    /// Agent type: "claude" or "gemini"
    agent_type: String,
}

type SharedState = Arc<Mutex<SharedData>>;

/// Discord message length limit
const DISCORD_MSG_LIMIT: usize = 2000;

/// Compute a short hash key from the bot token (first 16 chars of SHA-256 hex)
fn discord_token_hash(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let result = hasher.finalize();
    format!("dc_{}", hex::encode(&result[..8])) // prefix with dc_ to distinguish from Telegram
}

/// TypeMapKey for storing shared state in serenity's data map
struct BotState;
impl TypeMapKey for BotState {
    type Value = SharedState;
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // Ignore messages from bots (including ourselves)
        if msg.author.bot {
            return;
        }

        let state = {
            let data = ctx.data.read().await;
            match data.get::<BotState>() {
                Some(s) => s.clone(),
                None => return,
            }
        };

        if let Err(e) = handle_message(&ctx, &msg, &state).await {
            let ts = chrono::Local::now().format("%H:%M:%S");
            println!("  [{ts}]   ⚠ Discord error: {e}");
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("  ✓ Bot connected as {} — Listening for messages", ready.user.name);
    }
}

/// Entry point: start the Discord bot
pub async fn run_bot(token: &str, allowed_channel_id: Option<u64>, agent_type: &str) {
    let bot_settings = bot_common::load_bot_settings(&discord_token_hash(token));

    if let Some(cid) = allowed_channel_id {
        println!("  ✓ Channel ID restriction: {cid}");
    } else {
        match bot_settings.owner_user_id {
            Some(owner_id) => println!("  ✓ Owner: {owner_id}"),
            None => println!("  ⚠ No owner registered — first user will be registered as owner"),
        }
    }

    let shared_state: SharedState = Arc::new(Mutex::new(SharedData {
        sessions: HashMap::new(),
        settings: bot_settings,
        cancel_tokens: HashMap::new(),
        api_timestamps: HashMap::new(),
        allowed_channel_id,
        token: token.to_string(),
        agent_type: agent_type.to_string(),
    }));

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let client = Client::builder(token, intents)
        .event_handler(Handler)
        .type_map_insert::<BotState>(shared_state)
        .await;

    let mut client = match client {
        Ok(c) => c,
        Err(e) => {
            eprintln!("  ✗ Failed to create Discord client: {e}");
            return;
        }
    };

    if let Err(e) = client.start().await {
        eprintln!("  ✗ Discord client error: {e}");
    }
}

/// Route incoming messages to appropriate handlers
async fn handle_message(
    ctx: &Context,
    msg: &Message,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let channel_id = msg.channel_id;
    let user_name = &msg.author.name;
    let user_id = msg.author.id.get();
    let timestamp = chrono::Local::now().format("%H:%M:%S");

    // Auth check: --channel-id restriction takes priority over imprinting
    let (allowed_cid, _token) = {
        let data = state.lock().await;
        (data.allowed_channel_id, data.token.clone())
    };

    if let Some(allowed) = allowed_cid {
        if channel_id.get() != allowed {
            println!("  [{timestamp}] ✗ Rejected (channel:{}, user:{user_name}/{user_id}) — allowed channel: {allowed}", channel_id.get());
            return Ok(());
        }
    } else {
        // Imprinting mode
        let imprinted = {
            let mut data = state.lock().await;
            match data.settings.owner_user_id {
                None => {
                    data.settings.owner_user_id = Some(user_id);
                    bot_common::save_bot_settings(&discord_token_hash(&data.token), &data.settings, &[("platform", "discord")]);
                    println!("  [{timestamp}] ★ Owner registered: {user_name} (id:{user_id})");
                    true
                }
                Some(owner_id) => {
                    if user_id != owner_id {
                        println!("  [{timestamp}] ✗ Rejected: {user_name} (id:{user_id})");
                        return Ok(());
                    }
                    false
                }
            }
        };
        if imprinted {
            // Owner registration logged to console only
        }
    }

    let user_display = format!("{user_name}({user_id})");

    // Handle file attachments
    if !msg.attachments.is_empty() {
        println!("  [{timestamp}] ◀ [{user_display}] Upload: {} file(s)", msg.attachments.len());
        handle_file_upload(ctx, msg, state).await?;
        println!("  [{timestamp}] ▶ [{user_display}] Upload complete");
        return Ok(());
    }

    let text = msg.content.clone();
    if text.is_empty() {
        return Ok(());
    }

    let preview = truncate_str(&text, 60);

    // Auto-restore session from bot_settings.json if not in memory
    if !text.starts_with("/start") {
        let mut data = state.lock().await;
        if !data.sessions.contains_key(&channel_id) {
            if let Some(last_path) = data.settings.last_sessions.get(&channel_id.get().to_string()).cloned() {
                if Path::new(&last_path).is_dir() {
                    let existing = bot_common::load_existing_session(&last_path);
                    let session = data.sessions.entry(channel_id).or_insert_with(|| ChannelSession {
                        session_id: None,
                        current_path: None,
                        history: Vec::new(),
                        pending_uploads: Vec::new(),
                        cleared: false,
                    });
                    session.current_path = Some(last_path.clone());
                    if let Some((session_data, _)) = existing {
                        session.session_id = Some(session_data.session_id.clone());
                        session.history = session_data.history.clone();
                    }
                    let ts = chrono::Local::now().format("%H:%M:%S");
                    println!("  [{ts}] ↻ [{user_display}] Auto-restored session: {last_path}");
                }
            }
        }
    }

    // Block all messages except /stop while an AI request is in progress
    if !text.starts_with("/stop") {
        let data = state.lock().await;
        if data.cancel_tokens.contains_key(&channel_id) {
            drop(data);
            rate_limit_wait(state, channel_id).await;
            channel_id.say(&ctx.http, "AI request in progress. Use /stop to cancel.").await?;
            return Ok(());
        }
    }

    if text.starts_with("/stop") {
        println!("  [{timestamp}] ◀ [{user_display}] /stop");
        handle_stop_command(ctx, channel_id, state).await?;
    } else if text.starts_with("/help") {
        println!("  [{timestamp}] ◀ [{user_display}] /help");
        handle_help_command(ctx, channel_id, state).await?;
    } else if text.starts_with("/start") {
        println!("  [{timestamp}] ◀ [{user_display}] /start");
        handle_start_command(ctx, channel_id, &text, state).await?;
    } else if text.starts_with("/clear") {
        println!("  [{timestamp}] ◀ [{user_display}] /clear");
        handle_clear_command(ctx, channel_id, state).await?;
        println!("  [{timestamp}] ▶ [{user_display}] Session cleared");
    } else if text.starts_with("/pwd") {
        println!("  [{timestamp}] ◀ [{user_display}] /pwd");
        handle_pwd_command(ctx, channel_id, state).await?;
    } else if text.starts_with("/down") {
        println!("  [{timestamp}] ◀ [{user_display}] /down {}", text.strip_prefix("/down").unwrap_or("").trim());
        handle_down_command(ctx, channel_id, &text, state).await?;
    } else if text.starts_with("/availabletools") {
        println!("  [{timestamp}] ◀ [{user_display}] /availabletools");
        handle_availabletools_command(ctx, channel_id, state).await?;
    } else if text.starts_with("/allowedtools") {
        println!("  [{timestamp}] ◀ [{user_display}] /allowedtools");
        handle_allowedtools_command(ctx, channel_id, state).await?;
    } else if text.starts_with("/allowed") {
        println!("  [{timestamp}] ◀ [{user_display}] /allowed {}", text.strip_prefix("/allowed").unwrap_or("").trim());
        handle_allowed_command(ctx, channel_id, &text, state).await?;
    } else if text.starts_with('!') {
        println!("  [{timestamp}] ◀ [{user_display}] Shell: {preview}");
        handle_shell_command(ctx, channel_id, &text, state).await?;
        println!("  [{timestamp}] ▶ [{user_display}] Shell done");
    } else {
        println!("  [{timestamp}] ◀ [{user_display}] {preview}");
        handle_text_message(ctx, channel_id, &text, msg.id, state).await?;
    }

    Ok(())
}

/// Handle /help command
async fn handle_help_command(
    ctx: &Context,
    channel_id: ChannelId,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let help = "\
**aimi Discord Bot**
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
Any other message is sent to Claude AI.
AI can read, edit, and run commands in your session.

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
async fn handle_start_command(
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
        let workspace_dir = home.join(".aimi").join("workspace");
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
async fn handle_clear_command(
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
async fn handle_pwd_command(
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
async fn handle_stop_command(
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
async fn handle_down_command(
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
async fn handle_file_upload(
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
async fn handle_shell_command(
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
async fn handle_availabletools_command(
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
async fn handle_allowedtools_command(
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
async fn handle_allowed_command(
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

/// Handle regular text messages - send to Claude AI
async fn handle_text_message(
    ctx: &Context,
    channel_id: ChannelId,
    user_text: &str,
    user_msg_id: serenity::model::id::MessageId,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Get session info, allowed tools, and pending uploads
    let (session_info, allowed_tools, pending_uploads, token) = {
        let mut data = state.lock().await;
        let info = data.sessions.get(&channel_id).and_then(|session| {
            session.current_path.as_ref().map(|_| {
                (session.session_id.clone(), session.current_path.clone().unwrap_or_default())
            })
        });
        let tools = data.settings.allowed_tools.clone();
        let uploads = data.sessions.get_mut(&channel_id)
            .map(|s| {
                s.cleared = false;
                std::mem::take(&mut s.pending_uploads)
            })
            .unwrap_or_default();
        let token = data.token.clone();
        (info, tools, uploads, token)
    };

    let (session_id, current_path) = match session_info {
        Some(info) => info,
        None => {
            rate_limit_wait(state, channel_id).await;
            channel_id.say(&ctx.http, "No active session. Use /start <path> first.").await?;
            return Ok(());
        }
    };

    // Send placeholder message
    rate_limit_wait(state, channel_id).await;
    let placeholder = channel_id.say(&ctx.http, "...").await?;
    let placeholder_msg_id = placeholder.id;

    // Sanitize input
    let sanitized_input = session::sanitize_user_input(user_text);

    // Prepend pending file upload records
    let context_prompt = if pending_uploads.is_empty() {
        sanitized_input
    } else {
        let upload_context = pending_uploads.join("\n");
        format!("{}\n\n{}", upload_context, sanitized_input)
    };

    // Build disabled tools notice
    let default_tools: std::collections::HashSet<&str> = DEFAULT_ALLOWED_TOOLS.iter().copied().collect();
    let allowed_set: std::collections::HashSet<&str> = allowed_tools.iter().map(|s| s.as_str()).collect();
    let disabled: Vec<&&str> = default_tools.iter().filter(|t| !allowed_set.contains(**t)).collect();
    let disabled_notice = if disabled.is_empty() {
        String::new()
    } else {
        let names: Vec<&str> = disabled.iter().map(|t| **t).collect();
        format!(
            "\n\nDISABLED TOOLS: The following tools have been disabled by the user: {}.\n\
             You MUST NOT attempt to use these tools. \
             If a user's request requires a disabled tool, do NOT proceed with the task. \
             Instead, clearly inform the user which tool is needed and that it is currently disabled. \
             Suggest they re-enable it with: /allowed +ToolName",
            names.join(", ")
        )
    };

    // Build system prompt with sendfile instructions
    // Discord bots send files directly via attachment API, no --sendfile equivalent needed
    let token_hash = discord_token_hash(&token);
    let system_prompt_owned = format!(
        "You are chatting with a user through Discord.\n\
         Current working directory: {}\n\n\
         When your work produces a file the user would want (generated code, reports, images, archives, etc.),\n\
         mention the file path so the user can retrieve it with /down <path>.\n\n\
         Always keep the user informed about what you are doing. \
         Briefly explain each step as you work (e.g. \"Reading the file...\", \"Creating the script...\", \"Running tests...\"). \
         The user cannot see your tool calls, so narrate your progress so they know what is happening.\n\n\
         DISCORD FORMATTING RULES:\n\
         - Your response is displayed on Discord (2000 char message limit, mobile users common).\n\
         - Keep text responses concise and well-structured.\n\
         - Use short paragraphs with blank lines between them.\n\
         - Use bullet lists (- item) instead of wide tables when possible.\n\
         - If you must show a table, keep columns narrow (< 50 chars total width) so it renders on mobile.\n\
         - Use `inline code` for file names, commands, and short values.\n\
         - Use code blocks (```language) for multi-line code. Always specify the language hint.\n\
         - Prefer summarized output over raw dumps. Show key results, not everything.\n\
         - Do NOT use headers (## Title) unless the response is very long — they are visually heavy on Discord.\n\
         - CRITICAL: When showing diff or patch output, ALWAYS wrap it in ```diff ... ``` code blocks.\n\
           Raw diff lines starting with - or + outside a code block become Discord bullet points.\n\
         - CRITICAL: When showing grep/search results, file listings, or command output with multiple lines,\n\
           ALWAYS wrap them in ``` ... ``` code blocks.\n\
         - Never output raw terminal/shell output as plain text — always use a code block.\n\n\
         IMPORTANT: The user is on Discord and CANNOT interact with any interactive prompts, dialogs, or confirmation requests. \
         All tools that require user interaction (such as AskUserQuestion, EnterPlanMode, ExitPlanMode) will NOT work. \
         Never use tools that expect user interaction. If you need clarification, just ask in plain text.\n\
         Token hash for reference: {}{}",
        current_path, token_hash, disabled_notice
    );

    // Create cancel token
    let cancel_token = Arc::new(CancelToken::new());
    {
        let mut data = state.lock().await;
        data.cancel_tokens.insert(channel_id, cancel_token.clone());
    }

    // Create channel for streaming
    let (tx, rx) = mpsc::channel();

    let session_id_clone = session_id.clone();
    let current_path_clone = current_path.clone();
    let cancel_token_clone = cancel_token.clone();

    // Get agent type from state
    let agent_type = {
        let data = state.lock().await;
        data.agent_type.clone()
    };

    // Run agent in a blocking thread
    tokio::task::spawn_blocking(move || {
        let result = match agent_type.as_str() {
            "gemini" => gemini::execute_command_streaming(
                &context_prompt,
                session_id_clone.as_deref(),
                &current_path_clone,
                tx.clone(),
                Some(&system_prompt_owned),
                Some(&allowed_tools),
                Some(cancel_token_clone),
            ),
            "codex" => codex::execute_command_streaming(
                &context_prompt,
                session_id_clone.as_deref(),
                &current_path_clone,
                tx.clone(),
                Some(&system_prompt_owned),
                Some(&allowed_tools),
                Some(cancel_token_clone),
            ),
            "opencode" => opencode::execute_command_streaming(
                &context_prompt,
                session_id_clone.as_deref(),
                &current_path_clone,
                tx.clone(),
                Some(&system_prompt_owned),
                Some(&allowed_tools),
                Some(cancel_token_clone),
            ),
            _ => claude::execute_command_streaming(
                &context_prompt,
                session_id_clone.as_deref(),
                &current_path_clone,
                tx.clone(),
                Some(&system_prompt_owned),
                Some(&allowed_tools),
                Some(cancel_token_clone),
            ),
        };

        if let Err(e) = result {
            let _ = tx.send(StreamMessage::Error { message: e });
        }
    });

    // Spawn the polling loop as a separate task
    let http = ctx.http.clone();
    let state_owned = state.clone();
    let user_text_owned = user_text.to_string();
    tokio::spawn(async move {
        const SPINNER: &[&str] = &[
            "Processing.",
            "Processing..",
            "Processing...",
        ];
        let mut full_response = String::new();
        let mut last_edit_text = String::new();
        let mut done = false;
        let mut cancelled = false;
        let mut new_session_id: Option<String> = None;
        let mut spin_idx: usize = 0;
        let mut last_tool_name = String::new();
        let mut last_file_path = String::new();

        while !done {
            if cancel_token.cancelled.load(Ordering::Relaxed) {
                cancelled = true;
                break;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(3000)).await;

            if cancel_token.cancelled.load(Ordering::Relaxed) {
                cancelled = true;
                break;
            }

            // Drain all available messages
            loop {
                match rx.try_recv() {
                    Ok(msg) => {
                        match msg {
                            StreamMessage::Init { session_id: sid } => {
                                new_session_id = Some(sid);
                            }
                            StreamMessage::Text { content } => {
                                full_response.push_str(&content);
                            }
                            StreamMessage::ToolUse { name, input } => {
                                let summary = format_tool_input(&name, &input);
                                let ts = chrono::Local::now().format("%H:%M:%S");
                                println!("  [{ts}]   ⚙ {name}: {}", truncate_str(&summary, 80));
                                // For multi-line summaries (e.g. Edit with inline diff), only put
                                // the first line in the blockquote. The rest (e.g. ```diff block)
                                // goes after a blank line so Discord exits the blockquote context
                                // and applies syntax highlighting to the code block.
                                if let Some(nl_pos) = summary.find('\n') {
                                    let first_line = &summary[..nl_pos];
                                    let rest = &summary[nl_pos + 1..];
                                    full_response.push_str(&format!("\n> ⚙️ {}\n", first_line));
                                    if !rest.is_empty() {
                                        full_response.push('\n');
                                        full_response.push_str(rest);
                                        full_response.push('\n');
                                    }
                                } else {
                                    full_response.push_str(&format!("\n> ⚙️ {}\n", summary));
                                }
                                // Extract file path for language detection in subsequent ToolResult
                                if matches!(name.as_str(), "Read" | "Write" | "Edit") {
                                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&input) {
                                        last_file_path = v.get("file_path")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                    }
                                } else {
                                    last_file_path.clear();
                                }
                                last_tool_name = name;
                            }
                            StreamMessage::ToolResult { content, is_error } => {
                                if is_error {
                                    let ts = chrono::Local::now().format("%H:%M:%S");
                                    println!("  [{ts}]   ✗ Error: {}", truncate_str(&content, 80));
                                }
                                let file_hint = if last_file_path.is_empty() { None } else { Some(last_file_path.as_str()) };
                                let formatted = formatter::format_tool_result(&content, is_error, &last_tool_name, file_hint);
                                if !formatted.is_empty() {
                                    full_response.push_str(&formatted);
                                }
                            }
                            StreamMessage::TaskNotification { summary, .. } => {
                                if !summary.is_empty() {
                                    full_response.push_str(&format!("\n> 📋 Task: {}\n", summary));
                                }
                            }
                            StreamMessage::Done { result, session_id: sid } => {
                                if !result.is_empty() && full_response.is_empty() {
                                    full_response = result;
                                }
                                if let Some(s) = sid {
                                    new_session_id = Some(s);
                                }
                                done = true;
                            }
                            StreamMessage::Error { message } => {
                                full_response = format!("Error: {}", message);
                                done = true;
                            }
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        done = true;
                        break;
                    }
                }
            }

            // Build display text with indicator
            let indicator = SPINNER[spin_idx % SPINNER.len()];
            spin_idx += 1;

            let display_text = if full_response.is_empty() {
                indicator.to_string()
            } else {
                let normalized = normalize_empty_lines(&full_response);
                let truncated = truncate_str(&normalized, DISCORD_MSG_LIMIT - 20);
                format!("{}\n\n{}", truncated, indicator)
            };

            if display_text != last_edit_text && !done {
                rate_limit_wait(&state_owned, channel_id).await;
                if let Err(e) = channel_id.edit_message(&http, placeholder_msg_id, EditMessage::new().content(&display_text)).await {
                    let ts = chrono::Local::now().format("%H:%M:%S");
                    println!("  [{ts}]   ⚠ edit_message failed (streaming): {e}");
                }
                last_edit_text = display_text;
            } else if !done {
                // Send typing indicator
                let _ = channel_id.broadcast_typing(&http).await;
            }
        }

        // Remove cancel token
        {
            let mut data = state_owned.lock().await;
            data.cancel_tokens.remove(&channel_id);
        }

        if cancelled {
            // Ensure child process is killed
            if let Ok(guard) = cancel_token.child_pid.lock() {
                if let Some(pid) = *guard {
                    #[cfg(unix)]
                    #[allow(unsafe_code)]
                    unsafe {
                        libc::kill(pid as libc::pid_t, libc::SIGTERM);
                    }
                }
            }

            let stopped_response = if full_response.trim().is_empty() {
                "[Stopped]".to_string()
            } else {
                let normalized = normalize_empty_lines(&full_response);
                format!("{}\n\n[Stopped]", normalized)
            };

            // Send final stopped response
            rate_limit_wait(&state_owned, channel_id).await;
            if stopped_response.len() <= DISCORD_MSG_LIMIT {
                let _ = channel_id.edit_message(&http, placeholder_msg_id, EditMessage::new().content(&stopped_response)).await;
            } else {
                // Delete placeholder and send as multiple messages
                let _ = channel_id.delete_message(&http, placeholder_msg_id).await;
                let _ = send_long_message_raw(&http, channel_id, &stopped_response, &state_owned).await;
            }

            let ts = chrono::Local::now().format("%H:%M:%S");
            println!("  [{ts}] ■ Stopped");

            // Record in history
            let mut data = state_owned.lock().await;
            if let Some(session) = data.sessions.get_mut(&channel_id) {
                if !session.cleared {
                    if let Some(sid) = new_session_id {
                        session.session_id = Some(sid);
                    }
                    session.history.push(HistoryItem {
                        item_type: HistoryType::User,
                        content: user_text_owned,
                    });
                    session.history.push(HistoryItem {
                        item_type: HistoryType::Assistant,
                        content: stopped_response,
                    });
                    bot_common::save_session_to_file(session.session_id.as_deref(), &session.history, &current_path);
                }
            }

            return;
        }

        // Final response
        if full_response.is_empty() {
            full_response = "(No response)".to_string();
        }

        let full_response = normalize_empty_lines(&full_response);
        let full_response = fix_diff_code_blocks(&full_response);

        rate_limit_wait(&state_owned, channel_id).await;
        if full_response.len() <= DISCORD_MSG_LIMIT {
            let _ = channel_id.edit_message(&http, placeholder_msg_id, EditMessage::new().content(&full_response)).await;
        } else {
            // Delete placeholder and send as multiple messages
            let _ = channel_id.delete_message(&http, placeholder_msg_id).await;
            let _ = send_long_message_raw(&http, channel_id, &full_response, &state_owned).await;
        }

        // Update session state
        {
            let mut data = state_owned.lock().await;
            if let Some(session) = data.sessions.get_mut(&channel_id) {
                if !session.cleared {
                    if let Some(sid) = new_session_id {
                        session.session_id = Some(sid);
                    }
                    session.history.push(HistoryItem {
                        item_type: HistoryType::User,
                        content: user_text_owned,
                    });
                    session.history.push(HistoryItem {
                        item_type: HistoryType::Assistant,
                        content: full_response,
                    });
                    bot_common::save_session_to_file(session.session_id.as_deref(), &session.history, &current_path);
                }
            }
        }

        // Send a reply to the user's original message so they get a notification
        rate_limit_wait(&state_owned, channel_id).await;
        let reply = CreateMessage::new()
            .content("✅ Done")
            .reference_message((channel_id, user_msg_id));
        let _ = channel_id.send_message(&http, reply).await;

        let ts = chrono::Local::now().format("%H:%M:%S");
        println!("  [{ts}] ▶ Response sent");
    });

    Ok(())
}

/// Per-channel rate limiter (1 second gap for Discord)
async fn rate_limit_wait(state: &SharedState, channel_id: ChannelId) {
    let min_gap = tokio::time::Duration::from_millis(1000);
    let sleep_until = {
        let mut data = state.lock().await;
        let last = data.api_timestamps.entry(channel_id).or_insert_with(||
            tokio::time::Instant::now() - tokio::time::Duration::from_secs(10)
        );
        let earliest_next = *last + min_gap;
        let now = tokio::time::Instant::now();
        let target = if earliest_next > now { earliest_next } else { now };
        *last = target;
        target
    };
    tokio::time::sleep_until(sleep_until).await;
}

/// Send a message that may exceed Discord's 2000 character limit
async fn send_long_message(
    ctx: &Context,
    channel_id: ChannelId,
    text: &str,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    send_long_message_raw(&ctx.http, channel_id, text, state).await
}

/// Detect if a text chunk ends inside an unclosed code block.
/// Returns the language hint (e.g. "diff", "rust") if a block is open, or None if not.
/// Parses line-by-line so it correctly handles nested/multiple code blocks.
fn unclosed_code_block_lang(text: &str) -> Option<String> {
    let mut open_lang: Option<String> = None;
    for line in text.lines() {
        // Code fence lines start with ``` (optionally preceded by spaces)
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if open_lang.is_some() {
                // Closing fence
                open_lang = None;
            } else {
                // Opening fence — capture language hint
                let lang = trimmed[3..].trim().to_string();
                open_lang = Some(lang);
            }
        }
    }
    open_lang
}

/// Send a long message using raw HTTP (for use in spawned tasks)
async fn send_long_message_raw(
    http: &serenity::http::Http,
    channel_id: ChannelId,
    text: &str,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if text.len() <= DISCORD_MSG_LIMIT {
        rate_limit_wait(state, channel_id).await;
        channel_id.say(http, text).await?;
        return Ok(());
    }

    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= DISCORD_MSG_LIMIT {
            rate_limit_wait(state, channel_id).await;
            channel_id.say(http, remaining).await?;
            break;
        }

        let safe_end = floor_char_boundary(remaining, DISCORD_MSG_LIMIT);

        // Try to split at a newline for cleaner breaks; avoid zero-length chunk
        let split_at = match remaining[..safe_end].rfind('\n') {
            Some(0) | None => safe_end,
            Some(pos) => pos,
        };

        let (chunk, rest) = remaining.split_at(split_at);

        // Check whether the chunk ends inside an unclosed code block
        let open_lang = unclosed_code_block_lang(chunk);
        let chunk_to_send = if open_lang.is_some() {
            format!("{}\n```", chunk)
        } else {
            chunk.to_string()
        };

        rate_limit_wait(state, channel_id).await;
        channel_id.say(http, &chunk_to_send).await?;

        remaining = rest.strip_prefix('\n').unwrap_or(rest);

        // If we force-closed an open code block, reopen it in the next message
        if let Some(lang_hint) = open_lang {
            if !remaining.is_empty() {
                let reopened = if lang_hint.is_empty() {
                    format!("```\n{}", remaining)
                } else {
                    format!("```{}\n{}", lang_hint, remaining)
                };
                return Box::pin(send_long_message_raw(http, channel_id, &reopened, state)).await;
            }
        }
    }

    Ok(())
}

/// Fix code blocks that contain diff content but use a non-diff language hint.
/// Discord only applies +/- coloring (green/red) when the code block uses ```diff.
/// Claude sometimes wraps diff-like content in ```kotlin, ```rust, etc.
fn fix_diff_code_blocks(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        // Look for code fence opening: ``` at start of line (possibly after whitespace)
        if let Some(fence_start) = find_code_fence(bytes, pos) {
            let fence_line_end = memchr_newline(bytes, fence_start);
            let backticks_end = fence_start + count_backticks(bytes, fence_start);
            let lang_hint = text[backticks_end..fence_line_end].trim();
            let backtick_count = backticks_end - fence_start;

            // Find the closing fence
            if let Some(close_start) = find_closing_fence(bytes, fence_line_end, backtick_count) {
                let block_content = &text[fence_line_end..close_start];
                let block_content = block_content.strip_prefix('\n').unwrap_or(block_content);
                let close_line_end = memchr_newline(bytes, close_start);

                // Only fix blocks with a non-diff language hint that contain diff content
                if !lang_hint.is_empty() && lang_hint != "diff" && formatter::is_diff_content(block_content) {
                    // Copy everything before this fence
                    result.push_str(&text[pos..fence_start]);
                    // Write corrected fence with diff hint + newline
                    for _ in 0..backtick_count {
                        result.push('`');
                    }
                    result.push_str("diff\n");
                    // Copy from content start to end of closing fence line
                    result.push_str(&text[fence_line_end..close_line_end]);
                    pos = close_line_end;
                } else {
                    // No change needed, copy through closing fence
                    result.push_str(&text[pos..close_line_end]);
                    pos = close_line_end;
                }
            } else {
                // No closing fence found, copy the fence line as-is
                result.push_str(&text[pos..fence_line_end]);
                pos = fence_line_end;
            }
        } else {
            // No more code fences, copy rest of text
            result.push_str(&text[pos..]);
            break;
        }
    }

    result
}

/// Find the next code fence (```) starting from `from`, at start of a line.
fn find_code_fence(bytes: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i < bytes.len() {
        // Must be at start of line (i==0 or preceded by \n)
        if i == 0 || bytes[i - 1] == b'\n' {
            // Skip optional leading whitespace
            let mut j = i;
            while j < bytes.len() && bytes[j] == b' ' {
                j += 1;
            }
            if j + 2 < bytes.len() && bytes[j] == b'`' && bytes[j + 1] == b'`' && bytes[j + 2] == b'`' {
                return Some(j);
            }
        }
        // Advance to next line
        match bytes[i..].iter().position(|&b| b == b'\n') {
            Some(nl) => i += nl + 1,
            None => break,
        }
    }
    None
}

fn count_backticks(bytes: &[u8], from: usize) -> usize {
    let mut count = 0;
    while from + count < bytes.len() && bytes[from + count] == b'`' {
        count += 1;
    }
    count
}

fn memchr_newline(bytes: &[u8], from: usize) -> usize {
    match bytes[from..].iter().position(|&b| b == b'\n') {
        Some(nl) => from + nl + 1,
        None => bytes.len(),
    }
}

fn find_closing_fence(bytes: &[u8], from: usize, backtick_count: usize) -> Option<usize> {
    let mut i = from;
    // Skip past opening line
    if i < bytes.len() && bytes[i] == b'\n' {
        i += 1;
    }
    while i < bytes.len() {
        // Must be at start of line
        let line_start = i;
        // Skip optional leading whitespace
        while i < bytes.len() && bytes[i] == b' ' {
            i += 1;
        }
        // Check for backticks
        let bt_start = i;
        let bt_count = count_backticks(bytes, i);
        if bt_count >= backtick_count {
            // Check rest of line is empty (or just whitespace)
            let rest_start = bt_start + bt_count;
            let line_end = bytes[rest_start..].iter().position(|&b| b == b'\n')
                .map(|p| rest_start + p)
                .unwrap_or(bytes.len());
            let rest = &bytes[rest_start..line_end];
            if rest.iter().all(|&b| b == b' ' || b == b'\t') {
                return Some(line_start);
            }
        }
        // Advance to next line
        match bytes[i..].iter().position(|&b| b == b'\n') {
            Some(nl) => i += nl + 1,
            None => break,
        }
    }
    None
}

/// Format tool input: delegates to shared formatter (Discord uses short filenames)
fn format_tool_input(name: &str, input: &str) -> String {
    formatter::format_tool_input(name, input, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- normalize_tool_name ---

    #[test]
    fn test_normalize_tool_name_lowercase() {
        assert_eq!(normalize_tool_name("bash"), "Bash");
    }

    #[test]
    fn test_normalize_tool_name_uppercase() {
        assert_eq!(normalize_tool_name("BASH"), "Bash");
    }

    #[test]
    fn test_normalize_tool_name_empty() {
        assert_eq!(normalize_tool_name(""), "");
    }

    // --- tool_info ---

    #[test]
    fn test_tool_info_known() {
        let (desc, destructive) = tool_info("Bash");
        assert!(desc.contains("shell"));
        assert!(destructive);
    }

    #[test]
    fn test_tool_info_safe() {
        let (_, destructive) = tool_info("Read");
        assert!(!destructive);
    }

    #[test]
    fn test_tool_info_unknown() {
        let (desc, _) = tool_info("UnknownTool");
        assert_eq!(desc, "Custom tool");
    }

    // --- risk_badge ---

    #[test]
    fn test_risk_badge() {
        assert_eq!(risk_badge(true), "!!!");
        assert_eq!(risk_badge(false), "");
    }

    // --- find_code_fence ---

    #[test]
    fn test_find_code_fence_at_start() {
        let input = b"```rust\ncode\n```";
        assert_eq!(find_code_fence(input, 0), Some(0));
    }

    #[test]
    fn test_find_code_fence_after_newline() {
        let input = b"text\n```rust\ncode\n```";
        assert_eq!(find_code_fence(input, 0), Some(5));
    }

    #[test]
    fn test_find_code_fence_with_indent() {
        let input = b"text\n   ```rust\n";
        assert_eq!(find_code_fence(input, 0), Some(8));
    }

    #[test]
    fn test_find_code_fence_none() {
        let input = b"no fence here\njust text";
        assert_eq!(find_code_fence(input, 0), None);
    }

    // --- count_backticks ---

    #[test]
    fn test_count_backticks_three() {
        assert_eq!(count_backticks(b"```rest", 0), 3);
    }

    #[test]
    fn test_count_backticks_four() {
        assert_eq!(count_backticks(b"````rest", 0), 4);
    }

    #[test]
    fn test_count_backticks_zero() {
        assert_eq!(count_backticks(b"no backticks", 0), 0);
    }

    // --- memchr_newline ---

    #[test]
    fn test_memchr_newline_found() {
        assert_eq!(memchr_newline(b"hello\nworld", 0), 6);
    }

    #[test]
    fn test_memchr_newline_not_found() {
        assert_eq!(memchr_newline(b"no newline", 0), 10);
    }

    #[test]
    fn test_memchr_newline_from_offset() {
        assert_eq!(memchr_newline(b"aa\nbb\ncc", 3), 6);
    }

    // --- find_closing_fence ---

    #[test]
    fn test_find_closing_fence_basic() {
        let input = b"```rust\ncode line\n```\nafter";
        // from=7 (after "```rust"), backtick_count=3
        let result = find_closing_fence(input, 7, 3);
        assert!(result.is_some());
    }

    #[test]
    fn test_find_closing_fence_not_found() {
        let input = b"```rust\ncode line\nno close";
        let result = find_closing_fence(input, 7, 3);
        assert!(result.is_none());
    }

    // --- format_tool_input ---

    #[test]
    fn test_format_tool_input_bash() {
        let input = r#"{"command":"ls -la","description":"List files"}"#;
        let result = format_tool_input("Bash", input);
        assert_eq!(result, "List files: `ls -la`");
    }

    #[test]
    fn test_format_tool_input_read() {
        let input = r#"{"file_path":"/src/main.rs"}"#;
        let result = format_tool_input("Read", input);
        assert!(result.contains("main.rs"));
    }

    #[test]
    fn test_format_tool_input_glob() {
        let input = r#"{"pattern":"*.rs","path":"/src"}"#;
        let result = format_tool_input("Glob", input);
        assert_eq!(result, "Glob *.rs in /src");
    }

    #[test]
    fn test_format_tool_input_websearch() {
        let input = r#"{"query":"rust async"}"#;
        let result = format_tool_input("WebSearch", input);
        assert_eq!(result, "Search: rust async");
    }

    #[test]
    fn test_format_tool_input_invalid_json() {
        let result = format_tool_input("Bash", "not json");
        assert!(result.contains("Bash"));
    }

    // --- unclosed_code_block_lang ---

    #[test]
    fn test_no_code_block() {
        assert_eq!(unclosed_code_block_lang("just plain text\nno fences here"), None);
    }

    #[test]
    fn test_closed_code_block() {
        let text = "```rust\nfn main() {}\n```";
        assert_eq!(unclosed_code_block_lang(text), None);
    }

    #[test]
    fn test_open_code_block_with_lang() {
        let text = "some text\n```diff\n- old line\n+ new line";
        assert_eq!(unclosed_code_block_lang(text), Some("diff".to_string()));
    }

    #[test]
    fn test_open_code_block_no_lang() {
        let text = "prefix\n```\ncontent here";
        assert_eq!(unclosed_code_block_lang(text), Some("".to_string()));
    }

    #[test]
    fn test_multiple_blocks_last_open() {
        // Two complete blocks then one open
        let text = "```rust\nfn a() {}\n```\n```python\nprint()\n```\n```diff\n+added";
        assert_eq!(unclosed_code_block_lang(text), Some("diff".to_string()));
    }

    #[test]
    fn test_multiple_blocks_all_closed() {
        let text = "```rust\nfn a() {}\n```\n```python\nprint()\n```";
        assert_eq!(unclosed_code_block_lang(text), None);
    }

    #[test]
    fn test_empty_text() {
        assert_eq!(unclosed_code_block_lang(""), None);
    }

    // --- truncate_str ---

    #[test]
    fn test_truncate_str_short() {
        assert_eq!(truncate_str("hello", 100), "hello");
    }

    #[test]
    fn test_truncate_str_at_newline() {
        let s = "line1\nline2\nline3";
        // max_len = 10 → cuts at "line1\nline" → rfind('\n') = 5 → "line1"
        let result = truncate_str(s, 10);
        assert_eq!(result, "line1");
    }

    // --- normalize_empty_lines ---

    #[test]
    fn test_normalize_collapses_blank_lines() {
        let input = "a\n\n\n\nb";
        let result = normalize_empty_lines(input);
        assert_eq!(result, "a\n\nb");
    }

    #[test]
    fn test_normalize_single_blank_preserved() {
        let input = "a\n\nb";
        let result = normalize_empty_lines(input);
        assert_eq!(result, "a\n\nb");
    }

    // --- fix_diff_code_blocks ---

    #[test]
    fn test_fix_diff_blocks_kotlin_to_diff() {
        // Code block tagged as kotlin but content is diff → should become ```diff
        let input = "text\n```kotlin\n- old line 1\n- old line 2\n+ new line 1\n+ new line 2\n```\nmore";
        let result = fix_diff_code_blocks(input);
        assert!(result.contains("```diff\n"), "should change kotlin to diff: {}", result);
        assert!(!result.contains("```kotlin"), "should not contain kotlin hint");
    }

    #[test]
    fn test_fix_diff_blocks_already_diff() {
        // Code block already tagged as diff → no change
        let input = "```diff\n- old\n+ new\n+ added\n+ more\n```";
        let result = fix_diff_code_blocks(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_fix_diff_blocks_no_diff_content() {
        // Regular kotlin code → no change
        let input = "```kotlin\nval x = 1\nval y = 2\nfun main() {}\n```";
        let result = fix_diff_code_blocks(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_fix_diff_blocks_plain_no_lang() {
        // No language hint → no change (even if content is diff-like)
        let input = "```\n- old\n+ new\n- more old\n+ more new\n```";
        let result = fix_diff_code_blocks(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_fix_diff_blocks_preserves_surrounding() {
        let input = "before text\n```rust\n@@ -1,3 +1,4 @@\n- removed\n+ added\n context\n```\nafter text";
        let result = fix_diff_code_blocks(input);
        assert!(result.starts_with("before text\n"), "should preserve before text");
        assert!(result.ends_with("\nafter text"), "should preserve after text");
        assert!(result.contains("```diff\n"), "should change rust to diff");
    }

    #[test]
    fn test_fix_diff_blocks_multiple_blocks() {
        let input = "```kotlin\nval x = 1\n```\n```rust\n@@ -1,2 +1,2 @@\n- old\n+ new\n```\n```python\nprint()\n```";
        let result = fix_diff_code_blocks(input);
        assert!(result.contains("```kotlin\n"), "kotlin block should be unchanged");
        assert!(result.contains("```diff\n"), "rust block with diff content should become diff");
        assert!(result.contains("```python\n"), "python block should be unchanged");
    }
}
