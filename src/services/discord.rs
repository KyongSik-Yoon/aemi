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

use crate::services::claude::{self, CancelToken, StreamMessage, DEFAULT_ALLOWED_TOOLS};
use crate::services::session::{self, HistoryItem, HistoryType, SessionData};
use crate::services::formatter;

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

/// Bot-level settings persisted to disk
#[derive(Clone)]
struct BotSettings {
    allowed_tools: Vec<String>,
    /// channel_id (string) → last working directory path
    last_sessions: HashMap<String, String>,
    /// Discord user ID of the registered owner (imprinting auth)
    owner_user_id: Option<u64>,
}

impl Default for BotSettings {
    fn default() -> Self {
        Self {
            allowed_tools: DEFAULT_ALLOWED_TOOLS.iter().map(|s| s.to_string()).collect(),
            last_sessions: HashMap::new(),
            owner_user_id: None,
        }
    }
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

/// Path to bot settings file: ~/.cokacdir/bot_settings.json (shared with Telegram)
fn bot_settings_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".cokacdir").join("bot_settings.json"))
}

/// Load bot settings from bot_settings.json
fn load_bot_settings(token: &str) -> BotSettings {
    let Some(path) = bot_settings_path() else {
        return BotSettings::default();
    };
    let Ok(content) = fs::read_to_string(&path) else {
        return BotSettings::default();
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
        return BotSettings::default();
    };
    let key = discord_token_hash(token);
    let Some(entry) = json.get(&key) else {
        return BotSettings::default();
    };
    let owner_user_id = entry.get("owner_user_id").and_then(|v| v.as_u64());
    let Some(tools_arr) = entry.get("allowed_tools").and_then(|v| v.as_array()) else {
        return BotSettings { owner_user_id, ..BotSettings::default() };
    };
    let tools: Vec<String> = tools_arr
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    if tools.is_empty() {
        return BotSettings { owner_user_id, ..BotSettings::default() };
    }
    let last_sessions = entry.get("last_sessions")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();
    BotSettings { allowed_tools: tools, last_sessions, owner_user_id }
}

/// Save bot settings to bot_settings.json
fn save_bot_settings(token: &str, settings: &BotSettings) {
    let Some(path) = bot_settings_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut json: serde_json::Value = if let Ok(content) = fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    let key = discord_token_hash(token);
    let mut entry = serde_json::json!({
        "platform": "discord",
        "allowed_tools": settings.allowed_tools,
        "last_sessions": settings.last_sessions,
    });
    if let Some(owner_id) = settings.owner_user_id {
        entry["owner_user_id"] = serde_json::json!(owner_id);
    }
    json[key] = entry;
    if let Ok(s) = serde_json::to_string_pretty(&json) {
        let _ = fs::write(&path, s);
    }
}

/// Normalize tool name: first letter uppercase, rest lowercase
fn normalize_tool_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let mut chars = lower.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

/// All available tools with (description, is_destructive)
const ALL_TOOLS: &[(&str, &str, bool)] = &[
    ("Bash",            "Execute shell commands",                          true),
    ("Read",            "Read file contents from the filesystem",          false),
    ("Edit",            "Perform find-and-replace edits in files",         true),
    ("Write",           "Create or overwrite files",                       true),
    ("Glob",            "Find files by name pattern",                      false),
    ("Grep",            "Search file contents with regex",                 false),
    ("Task",            "Launch autonomous sub-agents for complex tasks",  true),
    ("TaskOutput",      "Retrieve output from background tasks",           false),
    ("TaskStop",        "Stop a running background task",                  false),
    ("WebFetch",        "Fetch and process web page content",              true),
    ("WebSearch",       "Search the web for up-to-date information",       true),
    ("NotebookEdit",    "Edit Jupyter notebook cells",                     true),
    ("Skill",           "Invoke slash-command skills",                     false),
    ("TaskCreate",      "Create a structured task in the task list",       false),
    ("TaskGet",         "Retrieve task details by ID",                     false),
    ("TaskUpdate",      "Update task status or details",                   false),
    ("TaskList",        "List all tasks and their status",                 false),
    ("AskUserQuestion", "Ask the user a question (interactive)",           false),
    ("EnterPlanMode",   "Enter planning mode (interactive)",               false),
    ("ExitPlanMode",    "Exit planning mode (interactive)",                false),
];

/// Tool info: (description, is_destructive)
fn tool_info(name: &str) -> (&'static str, bool) {
    ALL_TOOLS.iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, desc, destr)| (*desc, *destr))
        .unwrap_or(("Custom tool", false))
}

/// Format a risk badge for display
fn risk_badge(destructive: bool) -> &'static str {
    if destructive { "!!!" } else { "" }
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
pub async fn run_bot(token: &str, allowed_channel_id: Option<u64>) {
    let bot_settings = load_bot_settings(token);

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
                    save_bot_settings(&data.token, &data.settings);
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
                    let existing = load_existing_session(&last_path);
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
**cokacdir Discord Bot**
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
        let workspace_dir = home.join(".cokacdir").join("workspace");
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

    let existing = load_existing_session(&canonical_path);

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
        save_bot_settings(&token, &data.settings);
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
                save_session_to_file(session, &save_dir);
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
                let trimmed = stdout.trim_end();
                if formatter::is_diff_content(trimmed) {
                    parts.push(format!("```diff\n{}\n```", trimmed));
                } else {
                    parts.push(format!("```\n{}\n```", trimmed));
                }
            }
            if !stderr.is_empty() {
                parts.push(format!("stderr:\n```\n{}\n```", stderr.trim_end()));
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
                    save_bot_settings(&token, &data.settings);
                    format!("Added `{}`", tool_name)
                }
            }
            '-' => {
                let before_len = data.settings.allowed_tools.len();
                data.settings.allowed_tools.retain(|t| t != &tool_name);
                if data.settings.allowed_tools.len() < before_len {
                    save_bot_settings(&token, &data.settings);
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

    // Run Claude in a blocking thread
    tokio::task::spawn_blocking(move || {
        let result = claude::execute_command_streaming(
            &context_prompt,
            session_id_clone.as_deref(),
            &current_path_clone,
            tx.clone(),
            Some(&system_prompt_owned),
            Some(&allowed_tools),
            Some(cancel_token_clone),
        );

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
                                full_response.push_str(&format!("\n\n⚙️ {}\n", summary));
                                last_tool_name = name;
                            }
                            StreamMessage::ToolResult { content, is_error } => {
                                if is_error {
                                    let ts = chrono::Local::now().format("%H:%M:%S");
                                    println!("  [{ts}]   ✗ Error: {}", truncate_str(&content, 80));
                                }
                                let formatted = formatter::format_tool_result(&content, is_error, &last_tool_name, true);
                                if !formatted.is_empty() {
                                    full_response.push_str(&formatted);
                                }
                            }
                            StreamMessage::TaskNotification { summary, .. } => {
                                if !summary.is_empty() {
                                    full_response.push_str(&format!("\n[Task: {}]\n", summary));
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
                    save_session_to_file(session, &current_path);
                }
            }

            return;
        }

        // Final response
        if full_response.is_empty() {
            full_response = "(No response)".to_string();
        }

        let full_response = normalize_empty_lines(&full_response);

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
                    save_session_to_file(session, &current_path);
                }
            }
        }

        // Send a reply to the user's original message so they get a notification
        rate_limit_wait(&state_owned, channel_id).await;
        let reply = CreateMessage::new()
            .content("✅")
            .reference_message((channel_id, user_msg_id));
        let _ = channel_id.send_message(&http, reply).await;

        let ts = chrono::Local::now().format("%H:%M:%S");
        println!("  [{ts}] ▶ Response sent");
    });

    Ok(())
}

/// Load existing session from ai_sessions directory matching the given path
fn load_existing_session(current_path: &str) -> Option<(SessionData, std::time::SystemTime)> {
    let sessions_dir = session::ai_sessions_dir()?;

    if !sessions_dir.exists() {
        return None;
    }

    let mut matching_session: Option<(SessionData, std::time::SystemTime)> = None;

    if let Ok(entries) = fs::read_dir(&sessions_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(session_data) = serde_json::from_str::<SessionData>(&content) {
                        if session_data.current_path == current_path {
                            if let Ok(metadata) = path.metadata() {
                                if let Ok(modified) = metadata.modified() {
                                    match &matching_session {
                                        None => matching_session = Some((session_data, modified)),
                                        Some((_, latest_time)) if modified > *latest_time => {
                                            matching_session = Some((session_data, modified));
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    matching_session
}

/// Save session to file in the ai_sessions directory
fn save_session_to_file(session: &ChannelSession, current_path: &str) {
    let Some(ref session_id) = session.session_id else {
        return;
    };

    if session.history.is_empty() {
        return;
    }

    let Some(sessions_dir) = session::ai_sessions_dir() else {
        return;
    };

    if fs::create_dir_all(&sessions_dir).is_err() {
        return;
    }

    let saveable_history: Vec<HistoryItem> = session.history.iter()
        .filter(|item| !matches!(item.item_type, HistoryType::System))
        .cloned()
        .collect();

    if saveable_history.is_empty() {
        return;
    }

    let session_data = SessionData {
        session_id: session_id.clone(),
        history: saveable_history,
        current_path: current_path.to_string(),
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };

    let file_path = sessions_dir.join(format!("{}.json", session_id));

    if let Some(parent) = file_path.parent() {
        if parent != sessions_dir {
            return;
        }
    }

    if let Ok(json) = serde_json::to_string_pretty(&session_data) {
        let _ = fs::write(file_path, json);
    }
}

/// Find the largest byte index <= `index` that is a valid UTF-8 char boundary
fn floor_char_boundary(s: &str, index: usize) -> usize {
    if index >= s.len() {
        s.len()
    } else {
        let mut i = index;
        while !s.is_char_boundary(i) {
            i -= 1;
        }
        i
    }
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

        // Try to split at a newline for cleaner breaks
        let split_at = remaining[..safe_end]
            .rfind('\n')
            .unwrap_or(safe_end);

        // Handle code block continuity across splits
        let (chunk, rest) = remaining.split_at(split_at);

        // Check for unclosed code blocks
        let backtick_count = chunk.matches("```").count();
        let chunk_to_send = if backtick_count % 2 != 0 {
            // Unclosed code block - close it
            format!("{}\n```", chunk)
        } else {
            chunk.to_string()
        };

        rate_limit_wait(state, channel_id).await;
        channel_id.say(http, &chunk_to_send).await?;

        remaining = rest.strip_prefix('\n').unwrap_or(rest);

        // If we closed a code block, reopen it in the next chunk
        if backtick_count % 2 != 0 {
            remaining = remaining.strip_prefix("```").unwrap_or(remaining);
            // We'll prepend ``` if there's more content
            if !remaining.is_empty() {
                // Use a temporary buffer approach
                let reopened = format!("```\n{}", remaining);
                // Recursively handle the rest
                return Box::pin(send_long_message_raw(http, channel_id, &reopened, state)).await;
            }
        }
    }

    Ok(())
}

/// Normalize consecutive empty lines to maximum of one
fn normalize_empty_lines(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut prev_was_empty = false;

    for line in s.lines() {
        let is_empty = line.is_empty();
        if is_empty {
            if !prev_was_empty {
                result.push('\n');
            }
            prev_was_empty = true;
        } else {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(line);
            prev_was_empty = false;
        }
    }

    result
}

/// Truncate a string to max_len bytes, cutting at a safe UTF-8 char and line boundary
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }

    let safe_end = floor_char_boundary(s, max_len);
    let truncated = &s[..safe_end];
    if let Some(pos) = truncated.rfind('\n') {
        truncated[..pos].to_string()
    } else {
        truncated.to_string()
    }
}

/// Format tool input JSON into a human-readable summary (same as telegram.rs)
fn format_tool_input(name: &str, input: &str) -> String {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(input) else {
        return format!("{} {}", name, truncate_str(input, 200));
    };

    match name {
        "Bash" => {
            let desc = v.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let cmd = v.get("command").and_then(|v| v.as_str()).unwrap_or("");
            if !desc.is_empty() {
                format!("{}: `{}`", desc, truncate_str(cmd, 150))
            } else {
                format!("`{}`", truncate_str(cmd, 200))
            }
        }
        "Read" => {
            let fp = v.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
            format!("Read {}", fp)
        }
        "Write" => {
            let fp = v.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
            let content = v.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let lines = content.lines().count();
            if lines > 0 {
                format!("Write {} ({} lines)", fp, lines)
            } else {
                format!("Write {}", fp)
            }
        }
        "Edit" => {
            let fp = v.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
            let old_str = v.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
            let new_str = v.get("new_string").and_then(|v| v.as_str()).unwrap_or("");
            let replace_all = v.get("replace_all").and_then(|v| v.as_bool()).unwrap_or(false);
            formatter::format_edit_tool_use(fp, old_str, new_str, replace_all, true)
        }
        "Glob" => {
            let pattern = v.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let path = v.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if !path.is_empty() {
                format!("Glob {} in {}", pattern, path)
            } else {
                format!("Glob {}", pattern)
            }
        }
        "Grep" => {
            let pattern = v.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let path = v.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let output_mode = v.get("output_mode").and_then(|v| v.as_str()).unwrap_or("");
            if !path.is_empty() {
                if !output_mode.is_empty() {
                    format!("Grep \"{}\" in {} ({})", pattern, path, output_mode)
                } else {
                    format!("Grep \"{}\" in {}", pattern, path)
                }
            } else {
                format!("Grep \"{}\"", pattern)
            }
        }
        "NotebookEdit" => {
            let nb_path = v.get("notebook_path").and_then(|v| v.as_str()).unwrap_or("");
            let cell_id = v.get("cell_id").and_then(|v| v.as_str()).unwrap_or("");
            if !cell_id.is_empty() {
                format!("Notebook {} ({})", nb_path, cell_id)
            } else {
                format!("Notebook {}", nb_path)
            }
        }
        "WebSearch" => {
            let query = v.get("query").and_then(|v| v.as_str()).unwrap_or("");
            format!("Search: {}", query)
        }
        "WebFetch" => {
            let url = v.get("url").and_then(|v| v.as_str()).unwrap_or("");
            format!("Fetch {}", url)
        }
        "Task" => {
            let desc = v.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let subagent_type = v.get("subagent_type").and_then(|v| v.as_str()).unwrap_or("");
            if !subagent_type.is_empty() {
                format!("Task [{}]: {}", subagent_type, desc)
            } else {
                format!("Task: {}", desc)
            }
        }
        "TaskOutput" => {
            let task_id = v.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
            format!("Get task output: {}", task_id)
        }
        "TaskStop" => {
            let task_id = v.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
            format!("Stop task: {}", task_id)
        }
        "TodoWrite" => {
            if let Some(todos) = v.get("todos").and_then(|v| v.as_array()) {
                let pending = todos.iter().filter(|t| {
                    t.get("status").and_then(|s| s.as_str()) == Some("pending")
                }).count();
                let in_progress = todos.iter().filter(|t| {
                    t.get("status").and_then(|s| s.as_str()) == Some("in_progress")
                }).count();
                let completed = todos.iter().filter(|t| {
                    t.get("status").and_then(|s| s.as_str()) == Some("completed")
                }).count();
                format!("Todo: {} pending, {} in progress, {} completed", pending, in_progress, completed)
            } else {
                "Update todos".to_string()
            }
        }
        "Skill" => {
            let skill = v.get("skill").and_then(|v| v.as_str()).unwrap_or("");
            format!("Skill: {}", skill)
        }
        "AskUserQuestion" => {
            if let Some(questions) = v.get("questions").and_then(|v| v.as_array()) {
                if let Some(q) = questions.first() {
                    let question = q.get("question").and_then(|v| v.as_str()).unwrap_or("");
                    truncate_str(question, 200)
                } else {
                    "Ask user question".to_string()
                }
            } else {
                "Ask user question".to_string()
            }
        }
        "ExitPlanMode" => {
            "Exit plan mode".to_string()
        }
        "EnterPlanMode" => {
            "Enter plan mode".to_string()
        }
        "TaskCreate" => {
            let subject = v.get("subject").and_then(|v| v.as_str()).unwrap_or("");
            format!("Create task: {}", subject)
        }
        "TaskUpdate" => {
            let task_id = v.get("taskId").and_then(|v| v.as_str()).unwrap_or("");
            let status = v.get("status").and_then(|v| v.as_str()).unwrap_or("");
            if !status.is_empty() {
                format!("Update task {}: {}", task_id, status)
            } else {
                format!("Update task {}", task_id)
            }
        }
        "TaskGet" => {
            let task_id = v.get("taskId").and_then(|v| v.as_str()).unwrap_or("");
            format!("Get task: {}", task_id)
        }
        "TaskList" => {
            "List tasks".to_string()
        }
        _ => {
            format!("{} {}", name, truncate_str(input, 200))
        }
    }
}
