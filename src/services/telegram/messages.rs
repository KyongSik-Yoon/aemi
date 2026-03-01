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

/// Split HTML text into Telegram-sized chunks, handling <pre>/<code> tag
/// continuation across split points. Pure function for testability.
pub fn split_html_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut remaining = text;
    // Track the exact opening/closing tags for <pre> and optional <code class="...">
    // so split chunks produce valid HTML (e.g. <pre><code class="language-bash">)
    let mut pre_open_tag = String::new();
    let mut pre_close_tag = String::new();

    while !remaining.is_empty() {
        let in_pre = !pre_open_tag.is_empty();
        let tag_overhead = if in_pre {
            pre_open_tag.len() + pre_close_tag.len()
        } else {
            0
        };
        let effective_limit = max_len.saturating_sub(tag_overhead);

        if remaining.len() <= effective_limit {
            let mut chunk = String::new();
            if in_pre {
                chunk.push_str(&pre_open_tag);
            }
            chunk.push_str(remaining);
            chunks.push(chunk);
            break;
        }

        let safe_end = floor_char_boundary(remaining, effective_limit);
        let split_at = remaining[..safe_end]
            .rfind('\n')
            .unwrap_or(safe_end);

        let (raw_chunk, rest) = remaining.split_at(split_at);

        let mut chunk = String::new();
        if in_pre {
            chunk.push_str(&pre_open_tag);
        }
        chunk.push_str(raw_chunk);

        // Track unclosed <pre>/<code> tags to close/reopen across chunks
        let last_open = raw_chunk.rfind("<pre>");
        let last_close = raw_chunk.rfind("</pre>");
        let new_in_pre = match (last_open, last_close) {
            (Some(o), Some(c)) => o > c,
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (None, None) => in_pre,
        };
        if new_in_pre {
            // Extract the <code class="..."> tag if present after <pre>
            if let Some(open_pos) = last_open {
                let after_pre = &raw_chunk[open_pos + 5..]; // skip "<pre>"
                if after_pre.starts_with("<code") {
                    if let Some(gt_pos) = after_pre.find('>') {
                        let code_tag = &after_pre[..gt_pos + 1];
                        pre_open_tag = format!("<pre>{}", code_tag);
                        pre_close_tag = String::from("</code></pre>");
                    } else {
                        pre_open_tag = String::from("<pre>");
                        pre_close_tag = String::from("</pre>");
                    }
                } else {
                    pre_open_tag = String::from("<pre>");
                    pre_close_tag = String::from("</pre>");
                }
            }
            // else: no new <pre> in this chunk, keep tags from previous iteration
            chunk.push_str(&pre_close_tag);
        } else {
            pre_open_tag.clear();
            pre_close_tag.clear();
        }

        chunks.push(chunk);
        remaining = rest.strip_prefix('\n').unwrap_or(rest);
    }

    chunks
}

/// Send a message that may exceed Telegram's 4096 character limit
/// by splitting it into multiple messages, handling UTF-8 boundaries
/// and unclosed HTML tags (e.g. <pre>, <code>) across split points
pub async fn send_long_message(
    bot: &Bot,
    chat_id: ChatId,
    text: &str,
    parse_mode: Option<ParseMode>,
    state: &SharedState,
) -> ResponseResult<()> {
    let is_html = parse_mode.is_some();

    let chunks = if is_html {
        split_html_message(text, TELEGRAM_MSG_LIMIT)
    } else {
        // Plain text: simple split at newlines
        if text.len() <= TELEGRAM_MSG_LIMIT {
            vec![text.to_string()]
        } else {
            let mut result = Vec::new();
            let mut remaining = text;
            while !remaining.is_empty() {
                if remaining.len() <= TELEGRAM_MSG_LIMIT {
                    result.push(remaining.to_string());
                    break;
                }
                let safe_end = floor_char_boundary(remaining, TELEGRAM_MSG_LIMIT);
                let split_at = remaining[..safe_end]
                    .rfind('\n')
                    .unwrap_or(safe_end);
                let (chunk, rest) = remaining.split_at(split_at);
                result.push(chunk.to_string());
                remaining = rest.strip_prefix('\n').unwrap_or(rest);
            }
            result
        }
    };

    for chunk in &chunks {
        shared_rate_limit_wait(state, chat_id).await;
        let mut req = bot.send_message(chat_id, chunk);
        if let Some(mode) = parse_mode {
            req = req.parse_mode(mode);
        }
        req.await?;
    }

    Ok(())
}
