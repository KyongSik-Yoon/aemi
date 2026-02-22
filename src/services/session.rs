use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryItem {
    pub item_type: HistoryType,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoryType {
    User,
    Assistant,
    Error,
    System,
    ToolUse,      // Tool usage display (e.g., "[Bash]")
    ToolResult,   // Tool execution result
}

/// Session data structure for file persistence
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionData {
    pub session_id: String,
    pub history: Vec<HistoryItem>,
    pub current_path: String,
    pub created_at: String,
}

/// Get the AI sessions directory path (~/.cokacdir/ai_sessions)
pub fn ai_sessions_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".cokacdir").join("ai_sessions"))
}

/// Sanitize user input to prevent prompt injection attacks
/// Removes or escapes patterns that could be used to override AI instructions
pub fn sanitize_user_input(input: &str) -> String {
    let mut sanitized = input.to_string();

    // Remove common prompt injection patterns (case-insensitive)
    let dangerous_patterns = [
        "ignore previous instructions",
        "ignore all previous",
        "disregard previous",
        "forget previous",
        "system prompt",
        "you are now",
        "act as if",
        "pretend you are",
        "new instructions:",
        "[system]",
        "[admin]",
        "---begin",
        "---end",
    ];

    let lower_input = sanitized.to_lowercase();
    for pattern in dangerous_patterns {
        if lower_input.contains(pattern) {
            // Replace dangerous patterns with safe marker
            sanitized = sanitized.replace(pattern, "[filtered]");
            // Also handle case variations
            let pattern_lower = pattern.to_lowercase();
            let pattern_upper = pattern.to_uppercase();
            let pattern_title: String = pattern.chars().enumerate()
                .map(|(i, c)| if i == 0 { c.to_uppercase().next().unwrap_or(c) } else { c })
                .collect();
            sanitized = sanitized.replace(&pattern_lower, "[filtered]");
            sanitized = sanitized.replace(&pattern_upper, "[filtered]");
            sanitized = sanitized.replace(&pattern_title, "[filtered]");
        }
    }

    // Limit input length to prevent token exhaustion
    const MAX_INPUT_LENGTH: usize = 4000;
    if sanitized.len() > MAX_INPUT_LENGTH {
        safe_truncate(&mut sanitized, MAX_INPUT_LENGTH);
        sanitized.push_str("... [truncated]");
    }

    sanitized
}

/// Truncate string at a valid UTF-8 char boundary
fn safe_truncate(s: &mut String, max_bytes: usize) {
    if s.len() > max_bytes {
        let boundary = floor_char_boundary(s, max_bytes);
        s.truncate(boundary);
    }
}

/// Find the nearest char boundary at or before the given byte index
fn floor_char_boundary(s: &str, index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    let mut i = index;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}
