/// Shared formatting utilities for Telegram and Discord bot output.
///
/// All formatting functions output **markdown** format. The Telegram bot converts
/// to HTML via `markdown_to_telegram_html()` at the final rendering step.
/// Discord uses markdown natively.

/// Detect if content looks like unified diff output.
pub fn is_diff_content(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().take(40).collect();
    if lines.len() < 2 {
        return false;
    }

    let mut diff_indicators = 0;
    let mut has_hunk_header = false;

    for line in &lines {
        let trimmed = line.trim_start();
        if trimmed.starts_with("@@") && trimmed.contains("@@") {
            has_hunk_header = true;
            diff_indicators += 2;
        } else if trimmed.starts_with("diff --git") || trimmed.starts_with("---") || trimmed.starts_with("+++") {
            diff_indicators += 1;
        } else if trimmed.starts_with('+') && !trimmed.starts_with("+++") {
            diff_indicators += 1;
        } else if trimmed.starts_with('-') && !trimmed.starts_with("---") {
            diff_indicators += 1;
        }
    }

    has_hunk_header || diff_indicators >= 4
}

/// Detect if content looks like a table (pipe-delimited markdown table or aligned columns).
pub fn is_table_content(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().take(20).collect();
    if lines.len() < 2 {
        return false;
    }

    // Check for pipe-delimited markdown tables (| col1 | col2 |)
    let pipe_lines = lines.iter().filter(|l| {
        let trimmed = l.trim();
        trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.matches('|').count() >= 3
    }).count();

    if pipe_lines >= 2 {
        return true;
    }

    // Check for separator lines like |---|---|
    let has_separator = lines.iter().any(|l| {
        let trimmed = l.trim();
        trimmed.contains("---") && trimmed.contains('|')
    });

    has_separator && pipe_lines >= 1
}

/// Format a tool result with improved presentation.
/// Returns markdown string to be appended to full_response.
/// `is_discord`: true for Discord (uses ```diff), false for Telegram.
pub fn format_tool_result(content: &str, is_error: bool, last_tool_name: &str, is_discord: bool) -> String {
    if content.is_empty() && !is_error {
        return String::new();
    }

    let max_len: usize = 1500;

    if is_error {
        let truncated = smart_truncate(content, max_len);
        if truncated.contains('\n') {
            format!("\n❌\n```\n{}\n```\n", truncated)
        } else {
            format!("\n❌ `{}`\n", truncated)
        }
    } else {
        let is_diff = is_diff_content(content);
        let is_table = !is_diff && is_table_content(content);
        let truncated = smart_truncate_for_diff(content, max_len, is_diff);

        if is_diff {
            if is_discord {
                format!("\n```diff\n{}\n```\n", truncated)
            } else {
                format!("\n```\n{}\n```\n", truncated)
            }
        } else if is_table {
            // Tables always in code blocks to preserve alignment
            format!("\n```\n{}\n```\n", truncated)
        } else if truncated.contains('\n') {
            let lang = detect_language(last_tool_name, &truncated);
            if is_discord && !lang.is_empty() {
                format!("\n```{}\n{}\n```\n", lang, truncated)
            } else {
                format!("\n```\n{}\n```\n", truncated)
            }
        } else {
            format!("\n✅ `{}`\n", truncated)
        }
    }
}

/// Format Edit tool use with mini-diff for display.
/// Returns markdown string.
pub fn format_edit_tool_use(file_path: &str, old_string: &str, new_string: &str, replace_all: bool, is_discord: bool) -> String {
    // Extract short filename for display
    let short_name = file_path.rsplit('/').next().unwrap_or(file_path);
    let header = if replace_all {
        format!("Edit `{}` (replace all)", short_name)
    } else {
        format!("Edit `{}`", short_name)
    };

    // Only show diff if the strings are reasonably short
    let total_lines = old_string.lines().count() + new_string.lines().count();
    if total_lines == 0 || (old_string.len() > 600 && new_string.len() > 600) {
        return header;
    }

    let diff_text = build_edit_diff(old_string, new_string, 12);

    if is_discord {
        format!("{}\n```diff\n{}\n```", header, diff_text)
    } else {
        format!("{}\n```\n{}\n```", header, diff_text)
    }
}

/// Build a mini-diff view from Edit tool's old_string and new_string.
fn build_edit_diff(old_string: &str, new_string: &str, max_lines: usize) -> String {
    let old_lines: Vec<&str> = old_string.lines().collect();
    let new_lines: Vec<&str> = new_string.lines().collect();

    let mut diff_lines: Vec<String> = Vec::new();

    if old_lines.len() <= max_lines && new_lines.len() <= max_lines {
        for line in &old_lines {
            diff_lines.push(format!("- {}", line));
        }
        for line in &new_lines {
            diff_lines.push(format!("+ {}", line));
        }
    } else {
        let half = max_lines / 2;
        let old_show = old_lines.len().min(half);
        let new_show = new_lines.len().min(half);

        for line in old_lines.iter().take(old_show) {
            diff_lines.push(format!("- {}", line));
        }
        if old_lines.len() > old_show {
            diff_lines.push(format!("  ... ({} more lines removed)", old_lines.len() - old_show));
        }
        for line in new_lines.iter().take(new_show) {
            diff_lines.push(format!("+ {}", line));
        }
        if new_lines.len() > new_show {
            diff_lines.push(format!("  ... ({} more lines added)", new_lines.len() - new_show));
        }
    }

    diff_lines.join("\n")
}

/// Detect likely language from tool context for Discord code block hints.
fn detect_language(tool_name: &str, content: &str) -> &'static str {
    if is_diff_content(content) {
        return "diff";
    }

    match tool_name {
        "Bash" => {
            if content.contains("error[E") || content.contains("warning[") || content.contains("Compiling ") {
                "rust"
            } else if content.contains("SyntaxError") || content.contains("TypeError") || content.contains("node_modules") {
                "javascript"
            } else if content.contains("Traceback") || content.contains("IndentationError") {
                "python"
            } else if content.contains("PASS") && content.contains("FAIL") {
                // Test output (jest, cargo test, etc.)
                ""
            } else {
                ""
            }
        }
        "Read" => {
            // Try to detect from file content patterns
            if content.contains("fn ") && content.contains("let ") {
                "rust"
            } else if content.contains("function ") || content.contains("const ") || content.contains("import ") {
                "javascript"
            } else if content.contains("def ") && content.contains("self") {
                "python"
            } else {
                ""
            }
        }
        _ => "",
    }
}

/// Smart truncate for diff content - tries to keep complete hunks.
fn smart_truncate_for_diff(content: &str, max_len: usize, is_diff: bool) -> String {
    if content.len() <= max_len {
        return content.to_string();
    }

    if is_diff {
        // For diffs, try to break at hunk boundaries (@@ lines)
        let mut result = String::new();
        let mut current_len = 0;

        for line in content.lines() {
            let line_len = line.len() + 1;
            if current_len + line_len > max_len {
                result.push_str("\n... (truncated)");
                break;
            }
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(line);
            current_len += line_len;
        }

        result
    } else {
        smart_truncate(content, max_len)
    }
}

/// Smart truncate that tries to break at newline boundaries.
fn smart_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }

    let safe_end = floor_char_boundary(s, max_len);
    let truncated = &s[..safe_end];
    let result = if let Some(pos) = truncated.rfind('\n') {
        &truncated[..pos]
    } else {
        truncated
    };
    format!("{}...", result)
}

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
