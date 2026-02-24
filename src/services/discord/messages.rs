use serenity::model::id::ChannelId;
use serenity::prelude::*;

use crate::services::utils::floor_char_boundary;

use super::{SharedState, DISCORD_MSG_LIMIT};

/// Per-channel rate limiter (1 second gap for Discord)
pub async fn rate_limit_wait(state: &SharedState, channel_id: ChannelId) {
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
pub async fn send_long_message(
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
pub fn unclosed_code_block_lang(text: &str) -> Option<String> {
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
pub async fn send_long_message_raw(
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
