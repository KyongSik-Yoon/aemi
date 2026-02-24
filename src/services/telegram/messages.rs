use teloxide::prelude::*;
use teloxide::types::ParseMode;

use crate::services::utils::floor_char_boundary;

use super::{SharedState, TELEGRAM_MSG_LIMIT};

/// Shared per-chat rate limiter using reservation pattern.
/// Acquires the lock briefly to calculate and reserve the next API call slot,
/// then releases the lock and sleeps until the reserved time.
/// This ensures that even concurrent tasks for the same chat maintain 3s gaps.
pub async fn shared_rate_limit_wait(state: &SharedState, chat_id: ChatId) {
    let min_gap = tokio::time::Duration::from_millis(3000);
    let sleep_until = {
        let mut data = state.lock().await;
        let last = data.api_timestamps.entry(chat_id).or_insert_with(||
            tokio::time::Instant::now() - tokio::time::Duration::from_secs(10)
        );
        let earliest_next = *last + min_gap;
        let now = tokio::time::Instant::now();
        let target = if earliest_next > now { earliest_next } else { now };
        *last = target; // Reserve this slot
        target
    }; // Mutex released here
    tokio::time::sleep_until(sleep_until).await;
}

/// Escape special HTML characters for Telegram HTML parse mode
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Send a message that may exceed Telegram's 4096 character limit
/// by splitting it into multiple messages, handling UTF-8 boundaries
/// and unclosed HTML tags (e.g. <pre>) across split points
pub async fn send_long_message(
    bot: &Bot,
    chat_id: ChatId,
    text: &str,
    parse_mode: Option<ParseMode>,
    state: &SharedState,
) -> ResponseResult<()> {
    if text.len() <= TELEGRAM_MSG_LIMIT {
        shared_rate_limit_wait(state, chat_id).await;
        let mut req = bot.send_message(chat_id, text);
        if let Some(mode) = parse_mode {
            req = req.parse_mode(mode);
        }
        req.await?;
        return Ok(());
    }

    let is_html = parse_mode.is_some();
    let mut remaining = text;
    let mut in_pre = false;

    while !remaining.is_empty() {
        // Reserve space for tags we may need to add (<pre> + </pre> = 11 bytes)
        let tag_overhead = if is_html && in_pre { 11 } else { 0 };
        let effective_limit = TELEGRAM_MSG_LIMIT.saturating_sub(tag_overhead);

        if remaining.len() <= effective_limit {
            let mut chunk = String::new();
            if is_html && in_pre {
                chunk.push_str("<pre>");
            }
            chunk.push_str(remaining);

            shared_rate_limit_wait(state, chat_id).await;
            let mut req = bot.send_message(chat_id, &chunk);
            if let Some(mode) = parse_mode {
                req = req.parse_mode(mode);
            }
            req.await?;
            break;
        }

        // Find a safe UTF-8 char boundary, then find a newline before it
        let safe_end = floor_char_boundary(remaining, effective_limit);
        let split_at = remaining[..safe_end]
            .rfind('\n')
            .unwrap_or(safe_end);

        let (raw_chunk, rest) = remaining.split_at(split_at);

        let mut chunk = String::new();
        if is_html && in_pre {
            chunk.push_str("<pre>");
        }
        chunk.push_str(raw_chunk);

        // Track unclosed <pre> tags to close/reopen across chunks
        if is_html {
            let last_open = raw_chunk.rfind("<pre>");
            let last_close = raw_chunk.rfind("</pre>");
            in_pre = match (last_open, last_close) {
                (Some(o), Some(c)) => o > c,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (None, None) => in_pre,
            };
            if in_pre {
                chunk.push_str("</pre>");
            }
        }

        shared_rate_limit_wait(state, chat_id).await;
        let mut req = bot.send_message(chat_id, &chunk);
        if let Some(mode) = parse_mode {
            req = req.parse_mode(mode);
        }
        req.await?;

        // Skip the newline character at the split point
        remaining = rest.strip_prefix('\n').unwrap_or(rest);
    }

    Ok(())
}
