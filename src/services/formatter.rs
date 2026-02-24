/// Shared formatting utilities for Telegram and Discord bot output.
///
/// All formatting functions output **markdown** format. The Telegram bot converts
/// to HTML via `markdown_to_telegram_html()` at the final rendering step.
/// Discord uses markdown natively.

/// Strip ANSI terminal escape codes from a string.
/// Handles `ESC[...m` color/style sequences that appear in command output.
pub fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // ESC sequence: skip until alphabetic terminator
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                for nc in chars.by_ref() {
                    if nc.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            // Other ESC variants: just drop the ESC char
        } else {
            result.push(c);
        }
    }

    result
}

/// Map a file path's extension to a Discord/Telegram code block language hint.
pub fn detect_language_from_extension(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "rs" => "rust",
        "py" | "pyw" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "jsx" => "jsx",
        "kt" | "kts" => "kotlin",
        "java" => "java",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        "sh" | "bash" | "zsh" | "fish" => "bash",
        "yaml" | "yml" => "yaml",
        "json" | "jsonc" => "json",
        "toml" => "toml",
        "md" | "mdx" => "markdown",
        "sql" => "sql",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" | "sass" => "scss",
        "xml" | "plist" => "xml",
        "swift" => "swift",
        "dart" => "dart",
        "lua" => "lua",
        "php" => "php",
        "r" => "r",
        "gradle" => "groovy",
        _ => "",
    }
}

/// Try to convert Claude Code Read tool line-number format to cleaner `N: code` style.
/// Claude Code outputs lines as `     N→code` (U+2192 arrow). Returns None if the
/// content doesn't match that pattern (i.e. not a Read tool result).
///
/// Resilient to trailing non-matching lines (e.g. system reminders appended by
/// the API). As long as the first lines match the `N→` pattern, those are
/// reformatted and any remaining non-matching tail is dropped.
fn reformat_read_line_numbers(content: &str) -> Option<String> {
    const ARROW: char = '→'; // U+2192
    let mut result = String::with_capacity(content.len());
    let mut matched = 0usize;

    for line in content.lines() {
        let trimmed = line.trim_start();
        if let Some(arrow_pos) = trimmed.find(ARROW) {
            let num_part = &trimmed[..arrow_pos];
            if !num_part.is_empty() && num_part.chars().all(|c| c.is_ascii_digit()) {
                if matched > 0 {
                    result.push('\n');
                }
                matched += 1;
                result.push_str(num_part);
                result.push_str(": ");
                result.push_str(&trimmed[arrow_pos + ARROW.len_utf8()..]);
                continue;
            }
        }
        // Non-matching line: stop reformatting (trailing metadata / system tags)
        break;
    }

    if matched >= 2 { Some(result) } else { None }
}

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
/// `file_hint`: optional file path used to detect language from extension (e.g. "foo.rs" → "rust").
pub fn format_tool_result(content: &str, is_error: bool, last_tool_name: &str, file_hint: Option<&str>) -> String {
    if content.is_empty() && !is_error {
        return String::new();
    }

    // Strip ANSI escape codes so terminal color sequences don't leak into chat output
    let cleaned = strip_ansi_codes(content);
    let content = cleaned.as_str();

    let max_len: usize = 1500;

    if is_error {
        let truncated = smart_truncate(content, max_len);
        if truncated.contains('\n') {
            format!("\n❌\n```\n{}\n```\n", truncated)
        } else {
            format!("\n❌ `{}`\n", truncated)
        }
    } else {
        // Reformat "     N→code" line numbers to "N: code" for tools that return
        // file content with line numbers (Read results, Edit/Write result snippets).
        let reformatted = if matches!(last_tool_name, "Read" | "Edit" | "Write") {
            reformat_read_line_numbers(content)
        } else {
            None
        };
        let content = reformatted.as_deref().unwrap_or(content);

        let is_diff = is_diff_content(content);
        let is_table = !is_diff && is_table_content(content);
        let truncated = smart_truncate_for_diff(content, max_len, is_diff);

        if is_diff {
            format!("\n```diff\n{}\n```\n", truncated)
        } else if is_table {
            // Tables always in code blocks to preserve alignment
            format!("\n```\n{}\n```\n", truncated)
        } else if truncated.contains('\n') {
            // Language detection: file extension takes priority over content heuristics
            let lang = if let Some(path) = file_hint {
                let ext_lang = detect_language_from_extension(path);
                if !ext_lang.is_empty() { ext_lang } else { detect_language(last_tool_name, &truncated) }
            } else {
                detect_language(last_tool_name, &truncated)
            };
            if !lang.is_empty() {
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
pub fn format_edit_tool_use(file_path: &str, old_string: &str, new_string: &str, replace_all: bool) -> String {
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

    format!("{}\n```diff\n{}\n```", header, diff_text)
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- strip_ansi_codes ---

    #[test]
    fn test_strip_ansi_no_codes() {
        assert_eq!(strip_ansi_codes("hello world"), "hello world");
    }

    #[test]
    fn test_strip_ansi_empty() {
        assert_eq!(strip_ansi_codes(""), "");
    }

    #[test]
    fn test_strip_ansi_color_sequence() {
        // ESC[0;32m → ESC[0m (green color + reset)
        let input = "\x1b[0;32m→\x1b[0m Current branch: main";
        assert_eq!(strip_ansi_codes(input), "→ Current branch: main");
    }

    #[test]
    fn test_strip_ansi_multiple_sequences() {
        let input = "\x1b[1mBold\x1b[0m and \x1b[31mred\x1b[0m text";
        assert_eq!(strip_ansi_codes(input), "Bold and red text");
    }

    #[test]
    fn test_strip_ansi_preserves_newlines() {
        let input = "\x1b[32mline1\x1b[0m\nline2\n\x1b[33mline3\x1b[0m";
        assert_eq!(strip_ansi_codes(input), "line1\nline2\nline3");
    }

    #[test]
    fn test_strip_ansi_lone_esc_dropped() {
        // ESC not followed by '[' should just be dropped
        let input = "a\x1bb";
        assert_eq!(strip_ansi_codes(input), "ab");
    }

    // --- is_diff_content ---

    #[test]
    fn test_is_diff_with_hunk_header() {
        let diff = "@@ -1,3 +1,4 @@\n context\n-old line\n+new line";
        assert!(is_diff_content(diff));
    }

    #[test]
    fn test_is_diff_git_format() {
        let diff = "diff --git a/foo.rs b/foo.rs\n--- a/foo.rs\n+++ b/foo.rs\n@@ -1,2 +1,3 @@\n-old\n+new";
        assert!(is_diff_content(diff));
    }

    #[test]
    fn test_is_diff_plain_text() {
        let text = "This is just regular text.\nNothing special here.\nNo diff markers.";
        assert!(!is_diff_content(text));
    }

    #[test]
    fn test_is_diff_short_content() {
        assert!(!is_diff_content("single line"));
        assert!(!is_diff_content(""));
    }

    // --- detect_language_from_extension ---

    #[test]
    fn test_lang_from_ext_rust() {
        assert_eq!(detect_language_from_extension("src/main.rs"), "rust");
    }

    #[test]
    fn test_lang_from_ext_kotlin() {
        assert_eq!(detect_language_from_extension("/foo/Bar.kt"), "kotlin");
    }

    #[test]
    fn test_lang_from_ext_typescript() {
        assert_eq!(detect_language_from_extension("index.ts"), "typescript");
    }

    #[test]
    fn test_lang_from_ext_unknown() {
        assert_eq!(detect_language_from_extension("file.xyz"), "");
    }

    #[test]
    fn test_lang_from_ext_no_ext() {
        assert_eq!(detect_language_from_extension("Makefile"), "");
    }

    // --- reformat_read_line_numbers ---

    #[test]
    fn test_reformat_line_numbers_basic() {
        let input = "     1→fn main() {}\n     2→    println!(\"hi\");\n     3→}";
        let result = reformat_read_line_numbers(input).unwrap();
        assert_eq!(result, "1: fn main() {}\n2:     println!(\"hi\");\n3: }");
    }

    #[test]
    fn test_reformat_line_numbers_empty_line() {
        let input = "     1→first\n     2→\n     3→third";
        let result = reformat_read_line_numbers(input).unwrap();
        assert_eq!(result, "1: first\n2: \n3: third");
    }

    #[test]
    fn test_reformat_line_numbers_not_read_output() {
        let input = "regular text\nno line numbers here";
        assert!(reformat_read_line_numbers(input).is_none());
    }

    #[test]
    fn test_reformat_line_numbers_empty() {
        assert!(reformat_read_line_numbers("").is_none());
    }

    #[test]
    fn test_reformat_line_numbers_single_line_not_enough() {
        // Only 1 matching line is not enough (need >= 2)
        let input = "     1→only one line";
        assert!(reformat_read_line_numbers(input).is_none());
    }

    #[test]
    fn test_reformat_line_numbers_with_trailing_metadata() {
        // Simulates system-reminder or other metadata appended after file content
        let input = "     1→fn main() {}\n     2→}\n<system-reminder>some metadata</system-reminder>";
        let result = reformat_read_line_numbers(input).unwrap();
        assert_eq!(result, "1: fn main() {}\n2: }");
    }

    #[test]
    fn test_reformat_line_numbers_with_trailing_text() {
        // Simulates "... (N more lines)" truncation or other trailing text
        let input = "     10→    val name: String,\n     11→    val age: Int,\n... (20 more lines)";
        let result = reformat_read_line_numbers(input).unwrap();
        assert_eq!(result, "10:     val name: String,\n11:     val age: Int,");
    }

    // --- format_tool_result ---

    #[test]
    fn test_format_tool_result_empty() {
        assert_eq!(format_tool_result("", false, "Read", None), "");
    }

    #[test]
    fn test_format_tool_result_strips_ansi() {
        let input = "\x1b[32mok\x1b[0m";
        let result = format_tool_result(input, false, "Bash", None);
        assert!(!result.contains("\x1b"), "ANSI codes should be stripped");
        assert!(result.contains("ok"));
    }

    #[test]
    fn test_format_tool_result_error_single_line() {
        let result = format_tool_result("file not found", true, "Read", None);
        assert!(result.contains("❌"));
        assert!(result.contains("file not found"));
    }

    #[test]
    fn test_format_tool_result_diff_uses_lang_hint() {
        let diff = "@@ -1,2 +1,3 @@\n-old\n+new\n+added";
        let result = format_tool_result(diff, false, "Bash", None);
        assert!(result.contains("```diff"), "diff should use ```diff language hint");
    }

    #[test]
    fn test_format_tool_result_multiline_in_code_block() {
        // Plain multiline — no line-number format, no file hint
        let content = "line1\nline2\nline3";
        let result = format_tool_result(content, false, "Bash", None);
        assert!(result.contains("```"), "Multi-line should be in code block");
        assert!(result.contains("line1\nline2\nline3"));
    }

    #[test]
    fn test_format_tool_result_single_line_checkmark() {
        let result = format_tool_result("done", false, "Bash", None);
        assert!(result.contains("✅"));
        assert!(result.contains("`done`"));
    }

    #[test]
    fn test_format_tool_result_read_reformats_line_numbers() {
        let input = "     1→fn main() {}\n     2→}";
        let result = format_tool_result(input, false, "Read", Some("main.rs"));
        // Line numbers should be reformatted
        assert!(result.contains("1: fn main() {}"), "line numbers should be reformatted");
        // Language hint from extension should be applied
        assert!(result.contains("```rust"), "should use rust language hint from .rs extension");
    }

    #[test]
    fn test_format_tool_result_file_hint_sets_lang() {
        let content = "class Foo:\n    pass\n    return 1";
        let result = format_tool_result(content, false, "Read", Some("foo.py"));
        assert!(result.contains("```python"), "file hint .py should give python lang");
    }

    #[test]
    fn test_format_tool_result_edit_reformats_line_numbers() {
        // Edit tool results also contain cat-n style output with line numbers
        let input = "     10→    val profileId: Long = 0,\n     11→    val nickname: String = \"\",\n     12→)";
        let result = format_tool_result(input, false, "Edit", Some("ProfileModels.kt"));
        assert!(result.contains("10: "), "Edit tool result should reformat line numbers");
        assert!(!result.contains("→"), "Arrow should be replaced");
        assert!(result.contains("```kotlin"), "should use kotlin language hint from .kt extension");
    }

    #[test]
    fn test_format_tool_result_read_with_trailing_metadata() {
        // Read result with system reminder appended
        let input = "     1→import foo\n     2→import bar\n     3→\n<system-reminder>metadata</system-reminder>";
        let result = format_tool_result(input, false, "Read", Some("test.kt"));
        assert!(result.contains("1: import foo"), "line numbers should be reformatted despite trailing metadata");
        assert!(!result.contains("system-reminder"), "trailing metadata should be dropped");
    }
}
