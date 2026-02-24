mod commands;
mod chat;
mod messages;
mod formatting;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::id::ChannelId;
use serenity::prelude::*;

use crate::services::agent::CancelToken;
use crate::services::bot_common::{self, BotSettings};
use crate::services::session::HistoryItem;
use crate::services::utils::truncate_str;

/// Per-channel session state
pub(crate) struct ChannelSession {
    pub session_id: Option<String>,
    pub current_path: Option<String>,
    pub history: Vec<HistoryItem>,
    /// File upload records not yet sent to Claude AI.
    pub pending_uploads: Vec<String>,
    /// Set to true by /clear to prevent a racing polling loop from re-populating history.
    pub cleared: bool,
}

/// Shared state: per-channel sessions + bot settings
pub(crate) struct SharedData {
    pub sessions: HashMap<ChannelId, ChannelSession>,
    pub settings: BotSettings,
    /// Per-channel cancel tokens for stopping in-progress AI requests
    pub cancel_tokens: HashMap<ChannelId, Arc<CancelToken>>,
    /// Per-channel timestamp of the last Discord API call (for rate limiting)
    pub api_timestamps: HashMap<ChannelId, tokio::time::Instant>,
    /// If set, only messages from this channel ID are allowed (--channel-id parameter)
    pub allowed_channel_id: Option<u64>,
    /// Bot token (stored for settings persistence)
    pub token: String,
    /// Agent type: "claude" or "gemini"
    pub agent_type: String,
}

pub(crate) type SharedState = Arc<Mutex<SharedData>>;

/// Discord message length limit
pub(crate) const DISCORD_MSG_LIMIT: usize = 2000;

/// Compute a short hash key from the bot token (Discord: "dc_" prefix)
pub(crate) fn discord_token_hash(token: &str) -> String {
    bot_common::token_hash(token, Some("dc"))
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
        commands::handle_file_upload(ctx, msg, state).await?;
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
                if std::path::Path::new(&last_path).is_dir() {
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
            messages::rate_limit_wait(state, channel_id).await;
            channel_id.say(&ctx.http, "AI request in progress. Use /stop to cancel.").await?;
            return Ok(());
        }
    }

    if text.starts_with("/stop") {
        println!("  [{timestamp}] ◀ [{user_display}] /stop");
        commands::handle_stop_command(ctx, channel_id, state).await?;
    } else if text.starts_with("/help") {
        println!("  [{timestamp}] ◀ [{user_display}] /help");
        commands::handle_help_command(ctx, channel_id, state).await?;
    } else if text.starts_with("/start") {
        println!("  [{timestamp}] ◀ [{user_display}] /start");
        commands::handle_start_command(ctx, channel_id, &text, state).await?;
    } else if text.starts_with("/clear") {
        println!("  [{timestamp}] ◀ [{user_display}] /clear");
        commands::handle_clear_command(ctx, channel_id, state).await?;
        println!("  [{timestamp}] ▶ [{user_display}] Session cleared");
    } else if text.starts_with("/pwd") {
        println!("  [{timestamp}] ◀ [{user_display}] /pwd");
        commands::handle_pwd_command(ctx, channel_id, state).await?;
    } else if text.starts_with("/down") {
        println!("  [{timestamp}] ◀ [{user_display}] /down {}", text.strip_prefix("/down").unwrap_or("").trim());
        commands::handle_down_command(ctx, channel_id, &text, state).await?;
    } else if text.starts_with("/availabletools") {
        println!("  [{timestamp}] ◀ [{user_display}] /availabletools");
        commands::handle_availabletools_command(ctx, channel_id, state).await?;
    } else if text.starts_with("/allowedtools") {
        println!("  [{timestamp}] ◀ [{user_display}] /allowedtools");
        commands::handle_allowedtools_command(ctx, channel_id, state).await?;
    } else if text.starts_with("/allowed") {
        println!("  [{timestamp}] ◀ [{user_display}] /allowed {}", text.strip_prefix("/allowed").unwrap_or("").trim());
        commands::handle_allowed_command(ctx, channel_id, &text, state).await?;
    } else if text.starts_with('!') {
        println!("  [{timestamp}] ◀ [{user_display}] Shell: {preview}");
        commands::handle_shell_command(ctx, channel_id, &text, state).await?;
        println!("  [{timestamp}] ▶ [{user_display}] Shell done");
    } else {
        println!("  [{timestamp}] ◀ [{user_display}] {preview}");
        chat::handle_text_message(ctx, channel_id, &text, msg.id, state).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests;
