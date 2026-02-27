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
/// Returns `(language_hint, fence_length)` if a block is open, or None if not.
/// Fence-length-aware: a ```` block is NOT closed by inner ``` lines.
pub fn unclosed_code_block_lang(text: &str) -> Option<(String, usize)> {
    let mut open: Option<(String, usize)> = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("```") {
            continue;
        }
        let bt_count = trimmed.bytes().take_while(|&b| b == b'`').count();
        if let Some((_, open_len)) = &open {
            // Closing fence: must have >= opening fence backticks, rest is whitespace
            if bt_count >= *open_len && trimmed[bt_count..].trim().is_empty() {
                open = None;
            }
        } else {
            // Opening fence — capture language hint and fence length
            let lang = trimmed[bt_count..].trim().to_string();
            open = Some((lang, bt_count));
        }
    }
    open
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

    // Use an owned buffer so we can prepend fence reopeners without recursion
    let mut buf = String::new();
    let mut remaining: &str = text;

    while !remaining.is_empty() {
        // If we have a prepended fence from a previous iteration, use the owned buf
        let current = if buf.is_empty() { remaining } else { &buf };

        if current.len() <= DISCORD_MSG_LIMIT {
            rate_limit_wait(state, channel_id).await;
            channel_id.say(http, current).await?;
            break;
        }

        let safe_end = floor_char_boundary(current, DISCORD_MSG_LIMIT);

        // Try to split at a newline for cleaner breaks; avoid zero-length chunk
        let split_at = match current[..safe_end].rfind('\n') {
            Some(0) | None => safe_end,
            Some(pos) => pos,
        };

        let (chunk, rest) = current.split_at(split_at);

        // Check whether the chunk ends inside an unclosed code block
        let open_info = unclosed_code_block_lang(chunk);
        let chunk_to_send = if let Some((_, fence_len)) = &open_info {
            let fence: String = "`".repeat(*fence_len);
            format!("{}\n{}", chunk, fence)
        } else {
            chunk.to_string()
        };

        rate_limit_wait(state, channel_id).await;
        channel_id.say(http, &chunk_to_send).await?;

        let after = rest.strip_prefix('\n').unwrap_or(rest);

        // If we force-closed an open code block, reopen it in the next chunk
        if let Some((lang_hint, fence_len)) = open_info {
            if !after.is_empty() {
                let fence: String = "`".repeat(fence_len);
                buf = if lang_hint.is_empty() {
                    format!("{}\n{}", fence, after)
                } else {
                    format!("{}{}\n{}", fence, lang_hint, after)
                };
                // remaining is no longer used directly; buf holds the data
                remaining = "";
                continue;
            }
        }

        // No fence reopen needed — advance normally
        if buf.is_empty() {
            remaining = after;
        } else {
            buf = after.to_string();
            remaining = "";
        }
    }

    Ok(())
}
