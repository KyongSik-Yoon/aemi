use crate::services::bot_common::{normalize_tool_name, tool_info, risk_badge};
use crate::services::utils::{truncate_str, normalize_empty_lines};

use super::formatting::*;
use super::messages::unclosed_code_block_lang;

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

// --- tool_info ---

#[test]
fn test_tool_info_known() {
    let (desc, destructive) = tool_info("Bash");
    assert!(desc.contains("shell"));
    assert!(destructive);
}

#[test]
fn test_tool_info_safe() {
    let (_, destructive) = tool_info("Read");
    assert!(!destructive);
}

#[test]
fn test_tool_info_unknown() {
    let (desc, _) = tool_info("UnknownTool");
    assert_eq!(desc, "Custom tool");
}

// --- risk_badge ---

#[test]
fn test_risk_badge() {
    assert_eq!(risk_badge(true), "!!!");
    assert_eq!(risk_badge(false), "");
}

// --- find_code_fence ---

#[test]
fn test_find_code_fence_at_start() {
    let input = b"```rust\ncode\n```";
    assert_eq!(find_code_fence(input, 0), Some(0));
}

#[test]
fn test_find_code_fence_after_newline() {
    let input = b"text\n```rust\ncode\n```";
    assert_eq!(find_code_fence(input, 0), Some(5));
}

#[test]
fn test_find_code_fence_with_indent() {
    let input = b"text\n   ```rust\n";
    assert_eq!(find_code_fence(input, 0), Some(8));
}

#[test]
fn test_find_code_fence_none() {
    let input = b"no fence here\njust text";
    assert_eq!(find_code_fence(input, 0), None);
}

// --- count_backticks ---

#[test]
fn test_count_backticks_three() {
    assert_eq!(count_backticks(b"```rest", 0), 3);
}

#[test]
fn test_count_backticks_four() {
    assert_eq!(count_backticks(b"````rest", 0), 4);
}

#[test]
fn test_count_backticks_zero() {
    assert_eq!(count_backticks(b"no backticks", 0), 0);
}

// --- memchr_newline ---

#[test]
fn test_memchr_newline_found() {
    assert_eq!(memchr_newline(b"hello\nworld", 0), 6);
}

#[test]
fn test_memchr_newline_not_found() {
    assert_eq!(memchr_newline(b"no newline", 0), 10);
}

#[test]
fn test_memchr_newline_from_offset() {
    assert_eq!(memchr_newline(b"aa\nbb\ncc", 3), 6);
}

// --- find_closing_fence ---

#[test]
fn test_find_closing_fence_basic() {
    let input = b"```rust\ncode line\n```\nafter";
    // from=7 (after "```rust"), backtick_count=3
    let result = find_closing_fence(input, 7, 3);
    assert!(result.is_some());
}

#[test]
fn test_find_closing_fence_not_found() {
    let input = b"```rust\ncode line\nno close";
    let result = find_closing_fence(input, 7, 3);
    assert!(result.is_none());
}

// --- format_tool_input ---

#[test]
fn test_format_tool_input_bash() {
    let input = r#"{"command":"ls -la","description":"List files"}"#;
    let result = crate::services::formatter::format_tool_input("Bash", input, true);
    assert_eq!(result, "List files: `ls -la`");
}

#[test]
fn test_format_tool_input_read() {
    let input = r#"{"file_path":"/src/main.rs"}"#;
    let result = crate::services::formatter::format_tool_input("Read", input, true);
    assert!(result.contains("main.rs"));
}

#[test]
fn test_format_tool_input_glob() {
    let input = r#"{"pattern":"*.rs","path":"/src"}"#;
    let result = crate::services::formatter::format_tool_input("Glob", input, true);
    assert_eq!(result, "Glob *.rs in /src");
}

#[test]
fn test_format_tool_input_websearch() {
    let input = r#"{"query":"rust async"}"#;
    let result = crate::services::formatter::format_tool_input("WebSearch", input, true);
    assert_eq!(result, "Search: rust async");
}

#[test]
fn test_format_tool_input_invalid_json() {
    let result = crate::services::formatter::format_tool_input("Bash", "not json", true);
    assert!(result.contains("Bash"));
}

// --- unclosed_code_block_lang ---

#[test]
fn test_no_code_block() {
    assert_eq!(unclosed_code_block_lang("just plain text\nno fences here"), None);
}

#[test]
fn test_closed_code_block() {
    let text = "```rust\nfn main() {}\n```";
    assert_eq!(unclosed_code_block_lang(text), None);
}

#[test]
fn test_open_code_block_with_lang() {
    let text = "some text\n```diff\n- old line\n+ new line";
    assert_eq!(unclosed_code_block_lang(text), Some(("diff".to_string(), 3)));
}

#[test]
fn test_open_code_block_no_lang() {
    let text = "prefix\n```\ncontent here";
    assert_eq!(unclosed_code_block_lang(text), Some(("".to_string(), 3)));
}

#[test]
fn test_multiple_blocks_last_open() {
    // Two complete blocks then one open
    let text = "```rust\nfn a() {}\n```\n```python\nprint()\n```\n```diff\n+added";
    assert_eq!(unclosed_code_block_lang(text), Some(("diff".to_string(), 3)));
}

#[test]
fn test_multiple_blocks_all_closed() {
    let text = "```rust\nfn a() {}\n```\n```python\nprint()\n```";
    assert_eq!(unclosed_code_block_lang(text), None);
}

#[test]
fn test_empty_text() {
    assert_eq!(unclosed_code_block_lang(""), None);
}

// --- truncate_str ---

#[test]
fn test_truncate_str_short() {
    assert_eq!(truncate_str("hello", 100), "hello");
}

#[test]
fn test_truncate_str_at_newline() {
    let s = "line1\nline2\nline3";
    // max_len = 10 → cuts at "line1\nline" → rfind('\n') = 5 → "line1"
    let result = truncate_str(s, 10);
    assert_eq!(result, "line1");
}

// --- normalize_empty_lines ---

#[test]
fn test_normalize_collapses_blank_lines() {
    let input = "a\n\n\n\nb";
    let result = normalize_empty_lines(input);
    assert_eq!(result, "a\n\nb");
}

#[test]
fn test_normalize_single_blank_preserved() {
    let input = "a\n\nb";
    let result = normalize_empty_lines(input);
    assert_eq!(result, "a\n\nb");
}

// --- find_closing_fence edge cases ---

#[test]
fn test_find_closing_fence_four_backtick_blocks() {
    let input = b"````lang\ncode\n````\n";
    let result = find_closing_fence(input, 8, 4);
    assert!(result.is_some());
}

#[test]
fn test_find_closing_fence_mismatched_count() {
    // 3 backticks opening, but only 2 in "closing" line → should NOT match
    let input = b"```rust\ncode\n``\n";
    let result = find_closing_fence(input, 7, 3);
    assert!(result.is_none());
}

// --- unclosed_code_block_lang edge cases ---

#[test]
fn test_unclosed_code_block_nested_fences() {
    // Two opens and one close → one remains open
    let text = "```rust\nfn a() {}\n```\n```python\nprint()";
    assert_eq!(unclosed_code_block_lang(text), Some(("python".to_string(), 3)));
}

#[test]
fn test_unclosed_code_block_indented_fence() {
    // Indented ``` should still count
    let text = "   ```json\n{\"key\": 1}";
    assert_eq!(unclosed_code_block_lang(text), Some(("json".to_string(), 3)));
}

#[test]
fn test_unclosed_code_block_four_backtick_fence() {
    // ```` block should NOT be closed by inner ```
    let text = "````diff\n- old\n```\n+ new";
    assert_eq!(unclosed_code_block_lang(text), Some(("diff".to_string(), 4)));
}

#[test]
fn test_closed_four_backtick_fence() {
    // ```` block closed by ````
    let text = "````diff\n- old\n```\n+ new\n````";
    assert_eq!(unclosed_code_block_lang(text), None);
}

// --- fix_diff_code_blocks ---

#[test]
fn test_fix_diff_blocks_kotlin_to_diff() {
    // Code block tagged as kotlin but content is diff → should become ```diff
    let input = "text\n```kotlin\n- old line 1\n- old line 2\n+ new line 1\n+ new line 2\n```\nmore";
    let result = fix_diff_code_blocks(input);
    assert!(result.contains("```diff\n"), "should change kotlin to diff: {}", result);
    assert!(!result.contains("```kotlin"), "should not contain kotlin hint");
}

#[test]
fn test_fix_diff_blocks_already_diff() {
    // Code block already tagged as diff → no change
    let input = "```diff\n- old\n+ new\n+ added\n+ more\n```";
    let result = fix_diff_code_blocks(input);
    assert_eq!(result, input);
}

#[test]
fn test_fix_diff_blocks_no_diff_content() {
    // Regular kotlin code → no change
    let input = "```kotlin\nval x = 1\nval y = 2\nfun main() {}\n```";
    let result = fix_diff_code_blocks(input);
    assert_eq!(result, input);
}

#[test]
fn test_fix_diff_blocks_plain_no_lang() {
    // No language hint → no change (even if content is diff-like)
    let input = "```\n- old\n+ new\n- more old\n+ more new\n```";
    let result = fix_diff_code_blocks(input);
    assert_eq!(result, input);
}

#[test]
fn test_fix_diff_blocks_preserves_surrounding() {
    let input = "before text\n```rust\n@@ -1,3 +1,4 @@\n- removed\n+ added\n context\n```\nafter text";
    let result = fix_diff_code_blocks(input);
    assert!(result.starts_with("before text\n"), "should preserve before text");
    assert!(result.ends_with("\nafter text"), "should preserve after text");
    assert!(result.contains("```diff\n"), "should change rust to diff");
}

#[test]
fn test_fix_diff_blocks_multiple_blocks() {
    let input = "```kotlin\nval x = 1\n```\n```rust\n@@ -1,2 +1,2 @@\n- old\n+ new\n```\n```python\nprint()\n```";
    let result = fix_diff_code_blocks(input);
    assert!(result.contains("```kotlin\n"), "kotlin block should be unchanged");
    assert!(result.contains("```diff\n"), "rust block with diff content should become diff");
    assert!(result.contains("```python\n"), "python block should be unchanged");
}

// --- sanitize_inline_backticks ---

#[test]
fn test_sanitize_no_backticks() {
    let input = "just plain text\nnothing special";
    assert_eq!(sanitize_inline_backticks(input), input);
}

#[test]
fn test_sanitize_proper_code_block_untouched() {
    let input = "text\n```rust\nfn main() {}\n```\nmore";
    assert_eq!(sanitize_inline_backticks(input), input);
}

#[test]
fn test_sanitize_inline_triple_backtick() {
    let input = "use ```rust for code blocks";
    let result = sanitize_inline_backticks(input);
    assert!(!result.contains("```"), "inline ``` should be escaped");
    assert!(result.contains('\u{200B}'), "should contain zero-width space");
}

#[test]
fn test_sanitize_preserves_code_block_content() {
    // ``` inside a code block should NOT be escaped
    let input = "````rust\nformat!(\"{}\\n```\\n\", x)\n````";
    let result = sanitize_inline_backticks(input);
    assert_eq!(result, input, "content inside code blocks should be untouched");
}

#[test]
fn test_sanitize_mixed_content() {
    let input = "say ```rust to start\n```diff\n- old\n+ new\n```\ndone";
    let result = sanitize_inline_backticks(input);
    // First line: inline ``` escaped
    assert!(result.starts_with("say `\u{200B}``"));
    // Code block preserved
    assert!(result.contains("```diff\n- old\n+ new\n```"));
}
