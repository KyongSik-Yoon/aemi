/// Shared types and functions for Telegram and Discord bot implementations.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use sha2::{Sha256, Digest};

use crate::services::claude::DEFAULT_ALLOWED_TOOLS;
use crate::services::session::{self, HistoryItem, HistoryType, SessionData};

/// Bot-level settings persisted to disk
#[derive(Clone)]
pub struct BotSettings {
    pub allowed_tools: Vec<String>,
    /// channel/chat id (string) → last working directory path
    pub last_sessions: HashMap<String, String>,
    /// User ID of the registered owner (imprinting auth)
    pub owner_user_id: Option<u64>,
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

/// All available tools with (name, description, is_destructive)
pub const ALL_TOOLS: &[(&str, &str, bool)] = &[
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

/// Normalize tool name: first letter uppercase, rest lowercase
pub fn normalize_tool_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let mut chars = lower.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

/// Tool info: (description, is_destructive)
pub fn tool_info(name: &str) -> (&'static str, bool) {
    ALL_TOOLS.iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, desc, destr)| (*desc, *destr))
        .unwrap_or(("Custom tool", false))
}

/// Format a risk badge for display
pub fn risk_badge(destructive: bool) -> &'static str {
    if destructive { "!!!" } else { "" }
}

/// Compute a short hash key from the bot token (first 16 chars of SHA-256 hex).
/// Optional `prefix` is prepended with `_` separator (e.g., "dc" → "dc_<hash>").
pub fn token_hash(token: &str, prefix: Option<&str>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let result = hasher.finalize();
    let hash = hex::encode(&result[..8]); // 16 hex chars
    match prefix {
        Some(p) => format!("{}_{}", p, hash),
        None => hash,
    }
}

/// Path to bot settings file: ~/.aimi/bot_settings.json
pub fn bot_settings_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".aimi").join("bot_settings.json"))
}

/// Load bot settings from bot_settings.json using a pre-computed hash key.
pub fn load_bot_settings(hash_key: &str) -> BotSettings {
    let Some(path) = bot_settings_path() else {
        return BotSettings::default();
    };
    let Ok(content) = fs::read_to_string(&path) else {
        return BotSettings::default();
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
        return BotSettings::default();
    };
    let Some(entry) = json.get(hash_key) else {
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

/// Save bot settings to bot_settings.json.
/// `hash_key`: the computed hash key for this bot instance.
/// `platform_fields`: optional extra key-value pairs to store (e.g., `("token", token_value)` or `("platform", "discord")`).
pub fn save_bot_settings(
    hash_key: &str,
    settings: &BotSettings,
    platform_fields: &[(&str, &str)],
) {
    let Some(path) = bot_settings_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut json: serde_json::Value = if let Ok(content) = fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    let mut entry = serde_json::json!({
        "allowed_tools": settings.allowed_tools,
        "last_sessions": settings.last_sessions,
    });
    for &(key, value) in platform_fields {
        entry[key] = serde_json::json!(value);
    }
    if let Some(owner_id) = settings.owner_user_id {
        entry["owner_user_id"] = serde_json::json!(owner_id);
    }
    json[hash_key] = entry;
    if let Ok(s) = serde_json::to_string_pretty(&json) {
        let _ = fs::write(&path, s);
    }
}

/// Load the most recently modified session matching the given working directory path.
pub fn load_existing_session(current_path: &str) -> Option<(SessionData, std::time::SystemTime)> {
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

/// Save session to file in the ai_sessions directory.
/// Takes session fields directly to avoid coupling with platform-specific session structs.
pub fn save_session_to_file(
    session_id: Option<&str>,
    history: &[HistoryItem],
    current_path: &str,
) {
    let Some(session_id) = session_id else {
        return;
    };

    if history.is_empty() {
        return;
    }

    let Some(sessions_dir) = session::ai_sessions_dir() else {
        return;
    };

    if fs::create_dir_all(&sessions_dir).is_err() {
        return;
    }

    // Filter out system messages
    let saveable_history: Vec<HistoryItem> = history.iter()
        .filter(|item| !matches!(item.item_type, HistoryType::System))
        .cloned()
        .collect();

    if saveable_history.is_empty() {
        return;
    }

    let session_data = SessionData {
        session_id: session_id.to_string(),
        history: saveable_history,
        current_path: current_path.to_string(),
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };

    let file_path = sessions_dir.join(format!("{}.json", session_id));

    // Security: Verify the path is within sessions directory
    if let Some(parent) = file_path.parent() {
        if parent != sessions_dir {
            return;
        }
    }

    if let Ok(json) = serde_json::to_string_pretty(&session_data) {
        let _ = fs::write(file_path, json);
    }
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

    #[test]
    fn test_normalize_tool_name_mixed() {
        assert_eq!(normalize_tool_name("webFetch"), "Webfetch");
    }

    // --- tool_info ---

    #[test]
    fn test_tool_info_known_destructive() {
        let (desc, destructive) = tool_info("Bash");
        assert!(desc.contains("shell"));
        assert!(destructive);
    }

    #[test]
    fn test_tool_info_known_safe() {
        let (_, destructive) = tool_info("Read");
        assert!(!destructive);
    }

    #[test]
    fn test_tool_info_unknown() {
        let (desc, destructive) = tool_info("UnknownTool");
        assert_eq!(desc, "Custom tool");
        assert!(!destructive);
    }

    // --- risk_badge ---

    #[test]
    fn test_risk_badge_destructive() {
        assert_eq!(risk_badge(true), "!!!");
    }

    #[test]
    fn test_risk_badge_safe() {
        assert_eq!(risk_badge(false), "");
    }

    // --- token_hash ---

    #[test]
    fn test_token_hash_deterministic() {
        let h1 = token_hash("my-token", None);
        let h2 = token_hash("my-token", None);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_token_hash_length_no_prefix() {
        let h = token_hash("my-token", None);
        assert_eq!(h.len(), 16); // 8 bytes = 16 hex chars
    }

    #[test]
    fn test_token_hash_with_prefix() {
        let h = token_hash("my-token", Some("dc"));
        assert!(h.starts_with("dc_"));
        // "dc_" (3) + 16 hex chars = 19
        assert_eq!(h.len(), 19);
    }

    #[test]
    fn test_token_hash_different_tokens() {
        let h1 = token_hash("token-a", None);
        let h2 = token_hash("token-b", None);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_token_hash_same_token_different_prefix() {
        let h_none = token_hash("same-token", None);
        let h_dc = token_hash("same-token", Some("dc"));
        // The hash portion should be the same, just the prefix differs
        assert_eq!(h_dc.strip_prefix("dc_").unwrap(), h_none);
    }

    // --- bot_settings_path ---

    #[test]
    fn test_bot_settings_path_returns_some() {
        let path = bot_settings_path();
        assert!(path.is_some());
        let p = path.unwrap();
        assert!(p.ends_with("bot_settings.json"));
    }

    // --- BotSettings default ---

    #[test]
    fn test_bot_settings_default() {
        let settings = BotSettings::default();
        assert!(!settings.allowed_tools.is_empty());
        assert!(settings.last_sessions.is_empty());
        assert!(settings.owner_user_id.is_none());
    }
}
