use std::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use serenity::builder::{CreateMessage, EditMessage};
use serenity::model::id::ChannelId;
use serenity::prelude::*;

use crate::services::agent::{CancelToken, StreamMessage};
use crate::services::claude::{self, DEFAULT_ALLOWED_TOOLS};
use crate::services::gemini;
use crate::services::codex;
use crate::services::opencode;
use crate::services::oh_my_pi;
use crate::services::provider_common;
use crate::services::session::{self, HistoryItem, HistoryType};
use crate::services::formatter;
use crate::services::utils::{truncate_str, normalize_empty_lines};
use crate::services::bot_common;

use super::{SharedState, DISCORD_MSG_LIMIT, discord_token_hash};
use super::messages::{rate_limit_wait, send_long_message_raw, unclosed_code_block_lang};
use super::formatting::{fix_diff_code_blocks, sanitize_inline_backticks};

/// Format tool input: delegates to shared formatter (Discord uses short filenames)
fn format_tool_input(name: &str, input: &str) -> String {
    formatter::format_tool_input(name, input, true)
}

/// Handle regular text messages - send to AI agent
pub async fn handle_text_message(
    ctx: &Context,
    channel_id: ChannelId,
    user_text: &str,
    user_msg_id: serenity::model::id::MessageId,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Get session info, allowed tools, and pending uploads (drop lock before any await)
    let (session_info, allowed_tools, pending_uploads) = {
        let mut data = state.lock().await;
        let info = data.sessions.get(&channel_id).and_then(|session| {
            session.current_path.as_ref().map(|_| {
                (session.session_id.clone(), session.current_path.clone().unwrap_or_default())
            })
        });
        let tools = data.settings.allowed_tools.clone();
        // Drain pending uploads so they are sent to AI exactly once
        let uploads = data.sessions.get_mut(&channel_id)
            .map(|s| {
                s.cleared = false; // Reset cleared flag on new message
                std::mem::take(&mut s.pending_uploads)
            })
            .unwrap_or_default();
        (info, tools, uploads)
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

    // Prepend pending file upload records so AI knows about recently uploaded files
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

    // Get token hash for sendfile reference
    let token_hash = {
        let data = state.lock().await;
        discord_token_hash(&data.token)
    };

    // Build system prompt for Discord
    let system_prompt_owned = format!(
        "You are chatting with a user through Discord.\n\
         Current working directory: {}\n\n\
         When your work produces a file the user would want (generated code, reports, images, archives, etc.),\n\
         save it to the working directory and tell the user to use `/down <filepath>` to download it.\n\n\
         Always keep the user informed about what you are doing. \
         Briefly explain each step as you work (e.g. \"Reading the file...\", \"Creating the script...\", \"Running tests...\"). \
         The user cannot see your tool calls, so narrate your progress so they know what is happening.\n\n\
         IMPORTANT: The user is on Discord and CANNOT interact with any interactive prompts, dialogs, or confirmation requests. \
         All tools that require user interaction (such as AskUserQuestion, EnterPlanMode, ExitPlanMode) will NOT work. \
         Never use tools that expect user interaction. If you need clarification, just ask in plain text.\n\n\
         FORMATTING RULES FOR DISCORD:\n\
         - Use **bold** for emphasis\n\
         - Use `code` for inline code, file paths, commands\n\
         - Use ```language for code blocks (e.g. ```rust, ```bash)\n\
         - Use > for blockquotes\n\
         - Keep messages concise — Discord has a 2000 character limit per message\n\
         - Avoid headers (# Title) — they render as large text in Discord\n\
         - Use bullet lists (- item) for multiple items\n\
         - NEVER write triple backticks in regular text or inline code — Discord misinterprets them as code block markers. Say \"code block\" in words instead.\n\
         Token hash: {}{}",
        current_path, token_hash, disabled_notice
    );

    // Create cancel token for this request
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
            "oh-my-pi" => oh_my_pi::execute_command_streaming(
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

    // Spawn the polling loop as a separate task so the handler returns immediately.
    // This allows serenity to process subsequent messages (e.g. /stop).
    let http = ctx.http.clone();
    let state_owned = state.clone();
    let user_text_owned = user_text.to_string();
    let channel_id_num = channel_id.get();
    let placeholder_msg_id_num = placeholder_msg_id.get();
    let watcher_channel_id_num = channel_id_num;
    let watcher_placeholder_msg_id_num = placeholder_msg_id_num;

    let polling_handle = tokio::spawn(async move {
        provider_common::debug_log_for(
            "discord",
            &format!(
                "polling loop start channel_id={} placeholder_msg_id={}",
                channel_id_num, placeholder_msg_id_num
            ),
        );

        let mut full_response = String::new();
        let mut last_edit_text = String::new();
        let mut done = false;
        let mut cancelled = false;
        let mut new_session_id: Option<String> = None;
        let mut session_not_found = false;
        let mut spin_idx: usize = 0;
        let mut last_tool_name = String::new();
        let mut last_file_path = String::new();
        // Track current progress phase for contextual spinner
        let mut progress_phase = String::from("Thinking");
        // Track consecutive edit failures
        let mut consecutive_edit_failures: u32 = 0;

        while !done {
            // Check cancel token
            if cancel_token.cancelled.load(Ordering::Relaxed) {
                cancelled = true;
                break;
            }

            // Sleep 3s as polling interval
            tokio::time::sleep(tokio::time::Duration::from_millis(3000)).await;

            // Check cancel token again after sleep
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
                                progress_phase = String::from("Generating");
                            }
                            StreamMessage::ToolUse { name, input } => {
                                let summary = format_tool_input(&name, &input);
                                let ts = chrono::Local::now().format("%H:%M:%S");
                                println!("  [{ts}]   ⚙ {name}: {}", truncate_str(&summary, 80));
                                // Update progress phase with current tool name
                                progress_phase = format!("Using: {name}");

                                // Format tool use: header in blockquote, code blocks outside
                                let lines: Vec<&str> = summary.lines().collect();
                                if lines.len() <= 1 {
                                    full_response.push_str(&format!("\n\n> ⚙️ {}\n", summary));
                                } else {
                                    // First line is the header (blockquoted), rest is code block
                                    full_response.push_str(&format!("\n\n> ⚙️ {}\n", lines[0]));
                                    for line in &lines[1..] {
                                        full_response.push_str(line);
                                        full_response.push('\n');
                                    }
                                }

                                // Extract file path for language detection in subsequent ToolResult
                                if matches!(name.as_str(), "Read" | "Write" | "Edit") {
                                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&input) {
                                        last_file_path = v.get("file_path")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                    }
                                } else if name == "Grep" {
                                    last_file_path = formatter::extract_grep_file_hint(&input);
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
                                progress_phase = String::from("Thinking");
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
                                // Detect session-not-found errors to clear stale session_id
                                let lower = message.to_lowercase();
                                if lower.contains("session") && lower.contains("not found") {
                                    session_not_found = true;
                                }
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

            // Build display text with contextual progress indicator
            let dots = match spin_idx % 3 { 0 => ".", 1 => "..", _ => "..." };
            spin_idx += 1;
            let indicator = format!("{progress_phase}{dots}");

            let display_text = if full_response.is_empty() {
                indicator.to_string()
            } else {
                let normalized = normalize_empty_lines(&full_response);
                let truncated = truncate_str(&normalized, DISCORD_MSG_LIMIT - 50);
                // Close unclosed code blocks so Discord renders them properly
                if let Some((_, fence_len)) = unclosed_code_block_lang(&truncated) {
                    let fence: String = "`".repeat(fence_len);
                    format!("{}\n{}\n\n{}", truncated, fence, indicator)
                } else {
                    format!("{}\n\n{}", truncated, indicator)
                }
            };

            if display_text != last_edit_text && !done {
                // Skip edits if we've had too many consecutive failures
                if consecutive_edit_failures < 5 {
                    rate_limit_wait(&state_owned, channel_id).await;
                    let edit = EditMessage::new().content(&display_text);
                    match channel_id.edit_message(&http, placeholder_msg_id, edit).await {
                        Ok(_) => {
                            consecutive_edit_failures = 0;
                            last_edit_text = display_text;
                        }
                        Err(e) => {
                            consecutive_edit_failures += 1;
                            let ts = chrono::Local::now().format("%H:%M:%S");
                            println!("  [{ts}]   ⚠ edit_message failed ({consecutive_edit_failures}/5): {e}");
                        }
                    }
                }
            }
        }

        // Remove cancel token (processing is done)
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

            // Build stopped response: show partial content + [Stopped] indicator
            let stopped_response = if full_response.trim().is_empty() {
                "[Stopped]".to_string()
            } else {
                let normalized = normalize_empty_lines(&full_response);
                format!("{}\n\n[Stopped]", normalized)
            };

            // Update placeholder message with partial response
            rate_limit_wait(&state_owned, channel_id).await;
            if stopped_response.len() <= DISCORD_MSG_LIMIT {
                let edit = EditMessage::new().content(&stopped_response);
                if let Err(e) = channel_id.edit_message(&http, placeholder_msg_id, edit).await {
                    let ts = chrono::Local::now().format("%H:%M:%S");
                    println!("  [{ts}]   ⚠ edit_message failed (stopped): {e}");
                }
            } else {
                // Send as multiple messages for long stopped responses
                let _ = send_long_message_raw(&http, channel_id, &stopped_response, &state_owned).await;
                // Edit placeholder to indicate stopped
                rate_limit_wait(&state_owned, channel_id).await;
                let edit = EditMessage::new().content("[Stopped]");
                let _ = channel_id.edit_message(&http, placeholder_msg_id, edit).await;
            }

            let ts = chrono::Local::now().format("%H:%M:%S");
            println!("  [{ts}] ■ Stopped");

            // Record user message + stopped response in history
            // Skip if session was cleared while we were running (race with /clear)
            let mut data = state_owned.lock().await;
            if let Some(session) = data.sessions.get_mut(&channel_id) {
                if session.cleared {
                    // Session was cleared by /clear; do not re-populate
                } else {
                    if session_not_found {
                        session.session_id = None;
                    } else if let Some(sid) = new_session_id {
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
        let full_response = sanitize_inline_backticks(&full_response);

        rate_limit_wait(&state_owned, channel_id).await;

        if full_response.len() <= DISCORD_MSG_LIMIT {
            let edit = EditMessage::new().content(&full_response);
            if let Err(e) = channel_id.edit_message(&http, placeholder_msg_id, edit).await {
                let ts = chrono::Local::now().format("%H:%M:%S");
                println!("  [{ts}]   ⚠ edit_message failed (final): {e}");
            }
        } else {
            // Send as multiple messages for long responses, then edit placeholder
            let _ = send_long_message_raw(&http, channel_id, &full_response, &state_owned).await;
            // Edit placeholder to a minimal marker so it doesn't show stale content
            rate_limit_wait(&state_owned, channel_id).await;
            let edit = EditMessage::new().content("⬆️ (continued below)");
            let _ = channel_id.edit_message(&http, placeholder_msg_id, edit).await;
        }

        // Update session state: push user message + assistant response together
        // Skip if session was cleared while we were running (race with /clear)
        {
            let mut data = state_owned.lock().await;
            if let Some(session) = data.sessions.get_mut(&channel_id) {
                if session.cleared {
                    // Session was cleared by /clear; do not re-populate
                } else {
                    // Clear stale session_id on session-not-found error so next
                    // message starts a fresh session instead of repeating the error
                    if session_not_found {
                        session.session_id = None;
                    } else if let Some(sid) = new_session_id {
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

        // Send "Done" reply referencing user's original message
        rate_limit_wait(&state_owned, channel_id).await;
        let reply = CreateMessage::new()
            .content("✅ Done")
            .reference_message((channel_id, user_msg_id));
        let _ = channel_id.send_message(&http, reply).await;

        let ts = chrono::Local::now().format("%H:%M:%S");
        println!("  [{ts}] ▶ Response sent");
        provider_common::debug_log_for(
            "discord",
            &format!(
                "polling loop end channel_id={} cancelled={} done={}",
                channel_id_num, cancelled, done
            ),
        );
    });

    tokio::spawn(async move {
        if let Err(e) = polling_handle.await {
            provider_common::debug_log_for(
                "discord",
                &format!(
                    "polling loop join error channel_id={} placeholder_msg_id={}: {}",
                    watcher_channel_id_num, watcher_placeholder_msg_id_num, e
                ),
            );
            let ts = chrono::Local::now().format("%H:%M:%S");
            println!("  [{ts}]   ⚠ polling loop crashed: {e}");
        }
    });

    Ok(())
}
