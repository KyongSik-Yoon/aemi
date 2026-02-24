mod commands;
mod chat;
mod messages;
mod markdown;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use teloxide::prelude::*;

use crate::services::agent::CancelToken;
use crate::services::bot_common::{self, BotSettings};
use crate::services::utils::truncate_str;
use crate::services::session::HistoryItem;

/// Per-chat session state
pub(crate) struct ChatSession {
    pub session_id: Option<String>,
    pub current_path: Option<String>,
    pub history: Vec<HistoryItem>,
    /// File upload records not yet sent to Claude AI.
    /// Drained and prepended to the next user prompt so Claude knows about uploaded files.
    pub pending_uploads: Vec<String>,
    /// Set to true by /clear to prevent a racing polling loop from re-populating history.
    pub cleared: bool,
}

/// Shared state: per-chat sessions + bot settings
pub(crate) struct SharedData {
    pub sessions: HashMap<ChatId, ChatSession>,
    pub settings: BotSettings,
    /// Per-chat cancel tokens for stopping in-progress AI requests
    pub cancel_tokens: HashMap<ChatId, Arc<CancelToken>>,
    /// Message ID of the "Stopping..." message sent by /stop, so the polling loop can update it
    pub stop_message_ids: HashMap<ChatId, teloxide::types::MessageId>,
    /// Per-chat timestamp of the last Telegram API call (for rate limiting)
    pub api_timestamps: HashMap<ChatId, tokio::time::Instant>,
    /// If set, only messages from this chat ID are allowed (--chat-id parameter)
    pub allowed_chat_id: Option<i64>,
    /// Agent type: "claude" or "gemini"
    pub agent_type: String,
}

pub(crate) type SharedState = Arc<Mutex<SharedData>>;

/// Telegram message length limit
pub(crate) const TELEGRAM_MSG_LIMIT: usize = 4096;

/// Compute a short hash key from the bot token (Telegram: no prefix)
pub fn token_hash(token: &str) -> String {
    bot_common::token_hash(token, None)
}

/// Resolve a bot token from its hash by searching bot_settings.json
pub fn resolve_token_by_hash(hash: &str) -> Option<String> {
    use std::fs;
    let path = bot_common::bot_settings_path()?;
    let content = fs::read_to_string(&path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let obj = json.as_object()?;
    let entry = obj.get(hash)?;
    entry.get("token").and_then(|v| v.as_str()).map(String::from)
}

/// Entry point: start the Telegram bot with long polling
pub async fn run_bot(token: &str, allowed_chat_id: Option<i64>, agent_type: &str) {
    let bot = Bot::new(token);
    let bot_settings = bot_common::load_bot_settings(&token_hash(token));

    if let Some(cid) = allowed_chat_id {
        println!("  ✓ Chat ID restriction: {cid}");
    } else {
        match bot_settings.owner_user_id {
            Some(owner_id) => println!("  ✓ Owner: {owner_id}"),
            None => println!("  ⚠ No owner registered — first user will be registered as owner"),
        }
    }

    let state: SharedState = Arc::new(Mutex::new(SharedData {
        sessions: HashMap::new(),
        settings: bot_settings,
        cancel_tokens: HashMap::new(),
        stop_message_ids: HashMap::new(),
        api_timestamps: HashMap::new(),
        allowed_chat_id,
        agent_type: agent_type.to_string(),
    }));

    println!("  ✓ Bot connected — Listening for messages");

    let shared_state = state.clone();
    let token_owned = token.to_string();
    teloxide::repl(bot, move |bot: Bot, msg: Message| {
        let state = shared_state.clone();
        let token = token_owned.clone();
        async move {
            handle_message(bot, msg, state, &token).await
        }
    })
    .await;
}

/// Route incoming messages to appropriate handlers
async fn handle_message(
    bot: Bot,
    msg: Message,
    state: SharedState,
    token: &str,
) -> ResponseResult<()> {
    let chat_id = msg.chat.id;
    let raw_user_name = msg.from.as_ref()
        .map(|u| u.first_name.as_str())
        .unwrap_or("unknown");
    let timestamp = chrono::Local::now().format("%H:%M:%S");
    let user_id = msg.from.as_ref().map(|u| u.id.0);

    // Auth check: --chat-id restriction takes priority over imprinting
    let allowed_cid = {
        let data = state.lock().await;
        data.allowed_chat_id
    };

    if let Some(allowed) = allowed_cid {
        // --chat-id mode: only allow messages from the specified chat ID
        if chat_id.0 != allowed {
            let uid_str = user_id.map(|u| u.to_string()).unwrap_or_else(|| "?".to_string());
            println!("  [{timestamp}] ✗ Rejected (chat:{}, user:{raw_user_name}/{uid_str}) — allowed chat: {allowed}", chat_id.0);
            return Ok(());
        }
    } else {
        // Imprinting mode (original behavior)
        let Some(uid) = user_id else {
            // No user info (e.g. channel post) → reject
            return Ok(());
        };
        let imprinted = {
            let mut data = state.lock().await;
            match data.settings.owner_user_id {
                None => {
                    // Imprint: register first user as owner
                    data.settings.owner_user_id = Some(uid);
                    bot_common::save_bot_settings(&token_hash(token), &data.settings, &[("token", token)]);
                    println!("  [{timestamp}] ★ Owner registered: {raw_user_name} (id:{uid})");
                    true
                }
                Some(owner_id) => {
                    if uid != owner_id {
                        // Unregistered user → reject silently (log only)
                        println!("  [{timestamp}] ✗ Rejected: {raw_user_name} (id:{uid})");
                        return Ok(());
                    }
                    false
                }
            }
        };
        if imprinted {
            // Owner registration is logged to server console only
            // No response sent to the user
        }
    }

    let uid_display = user_id.map(|u| u.to_string()).unwrap_or_else(|| "?".to_string());
    let user_name = format!("{raw_user_name}({uid_display})");

    // Handle file/photo uploads
    if msg.document().is_some() || msg.photo().is_some() {
        let file_hint = if msg.document().is_some() { "document" } else { "photo" };
        println!("  [{timestamp}] ◀ [{user_name}] Upload: {file_hint}");
        let result = commands::handle_file_upload(&bot, chat_id, &msg, &state).await;
        println!("  [{timestamp}] ▶ [{user_name}] Upload complete");
        return result;
    }

    let Some(text) = msg.text() else {
        return Ok(());
    };

    let text = text.to_string();
    let preview = truncate_str(&text, 60);

    // Auto-restore session from bot_settings.json if not in memory
    if !text.starts_with("/start") {
        let mut data = state.lock().await;
        if !data.sessions.contains_key(&chat_id) {
            if let Some(last_path) = data.settings.last_sessions.get(&chat_id.0.to_string()).cloned() {
                if std::path::Path::new(&last_path).is_dir() {
                    let existing = bot_common::load_existing_session(&last_path);
                    let session = data.sessions.entry(chat_id).or_insert_with(|| ChatSession {
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
                    println!("  [{ts}] ↻ [{user_name}] Auto-restored session: {last_path}");
                }
            }
        }
    }

    // Block all messages except /stop while an AI request is in progress
    if !text.starts_with("/stop") {
        let data = state.lock().await;
        if data.cancel_tokens.contains_key(&chat_id) {
            drop(data);
            messages::shared_rate_limit_wait(&state, chat_id).await;
            bot.send_message(chat_id, "AI request in progress. Use /stop to cancel.")
                .await?;
            return Ok(());
        }
    }

    if text.starts_with("/stop") {
        println!("  [{timestamp}] ◀ [{user_name}] /stop");
        commands::handle_stop_command(&bot, chat_id, &state).await?;
    } else if text.starts_with("/help") {
        println!("  [{timestamp}] ◀ [{user_name}] /help");
        commands::handle_help_command(&bot, chat_id, &state).await?;
    } else if text.starts_with("/start") {
        println!("  [{timestamp}] ◀ [{user_name}] /start");
        commands::handle_start_command(&bot, chat_id, &text, &state, token).await?;
    } else if text.starts_with("/clear") {
        println!("  [{timestamp}] ◀ [{user_name}] /clear");
        commands::handle_clear_command(&bot, chat_id, &state).await?;
        println!("  [{timestamp}] ▶ [{user_name}] Session cleared");
    } else if text.starts_with("/pwd") {
        println!("  [{timestamp}] ◀ [{user_name}] /pwd");
        commands::handle_pwd_command(&bot, chat_id, &state).await?;
    } else if text.starts_with("/down") {
        println!("  [{timestamp}] ◀ [{user_name}] /down {}", text.strip_prefix("/down").unwrap_or("").trim());
        commands::handle_down_command(&bot, chat_id, &text, &state).await?;
    } else if text.starts_with("/availabletools") {
        println!("  [{timestamp}] ◀ [{user_name}] /availabletools");
        commands::handle_availabletools_command(&bot, chat_id, &state).await?;
    } else if text.starts_with("/allowedtools") {
        println!("  [{timestamp}] ◀ [{user_name}] /allowedtools");
        commands::handle_allowedtools_command(&bot, chat_id, &state).await?;
    } else if text.starts_with("/allowed") {
        println!("  [{timestamp}] ◀ [{user_name}] /allowed {}", text.strip_prefix("/allowed").unwrap_or("").trim());
        commands::handle_allowed_command(&bot, chat_id, &text, &state, token).await?;
    } else if text.starts_with('!') {
        println!("  [{timestamp}] ◀ [{user_name}] Shell: {preview}");
        commands::handle_shell_command(&bot, chat_id, &text, &state).await?;
        println!("  [{timestamp}] ▶ [{user_name}] Shell done");
    } else {
        println!("  [{timestamp}] ◀ [{user_name}] {preview}");
        chat::handle_text_message(&bot, chat_id, &text, msg.id, &state).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests;
