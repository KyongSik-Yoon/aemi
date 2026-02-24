use super::*;
use super::markdown::*;
use super::messages::html_escape;
use crate::services::utils::{floor_char_boundary, truncate_str, normalize_empty_lines};
use crate::services::bot_common::{normalize_tool_name, tool_info, risk_badge};

// --- token_hash ---

#[test]
fn test_token_hash_deterministic() {
    let hash1 = token_hash("test-token-123");
    let hash2 = token_hash("test-token-123");
    assert_eq!(hash1, hash2);
}

#[test]
fn test_token_hash_length() {
    let hash = token_hash("my-bot-token");
    assert_eq!(hash.len(), 16); // 8 bytes = 16 hex chars
}

#[test]
fn test_token_hash_different_tokens() {
    let hash1 = token_hash("token-a");
    let hash2 = token_hash("token-b");
    assert_ne!(hash1, hash2);
}

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
fn test_normalize_tool_name_mixed() {
    assert_eq!(normalize_tool_name("webFetch"), "Webfetch");
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
fn test_tool_info_safe_tool() {
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

// --- html_escape ---

#[test]
fn test_html_escape_basic() {
    assert_eq!(html_escape("<div>"), "&lt;div&gt;");
}

#[test]
fn test_html_escape_ampersand() {
    assert_eq!(html_escape("a & b"), "a &amp; b");
}

#[test]
fn test_html_escape_combined() {
    assert_eq!(html_escape("<a href=\"&\">"), "&lt;a href=\"&amp;\"&gt;");
}

#[test]
fn test_html_escape_no_escape_needed() {
    assert_eq!(html_escape("hello world"), "hello world");
}

// --- is_horizontal_rule ---

#[test]
fn test_horizontal_rule_dashes() {
    assert!(is_horizontal_rule("---"));
    assert!(is_horizontal_rule("-----"));
}

#[test]
fn test_horizontal_rule_stars() {
    assert!(is_horizontal_rule("***"));
    assert!(is_horizontal_rule("*****"));
}

#[test]
fn test_horizontal_rule_underscores() {
    assert!(is_horizontal_rule("___"));
}

#[test]
fn test_horizontal_rule_with_spaces() {
    assert!(is_horizontal_rule("- - -"));
    assert!(is_horizontal_rule("* * *"));
}

#[test]
fn test_horizontal_rule_too_short() {
    assert!(!is_horizontal_rule("--"));
    assert!(!is_horizontal_rule("**"));
}

#[test]
fn test_horizontal_rule_not_rule() {
    assert!(!is_horizontal_rule("hello"));
    assert!(!is_horizontal_rule(""));
}

// --- strip_heading ---

#[test]
fn test_strip_heading_h1() {
    assert_eq!(strip_heading("# Title"), Some("Title"));
}

#[test]
fn test_strip_heading_h3() {
    assert_eq!(strip_heading("### Section"), Some("Section"));
}

#[test]
fn test_strip_heading_h6() {
    assert_eq!(strip_heading("###### Deep"), Some("Deep"));
}

#[test]
fn test_strip_heading_too_many_hashes() {
    assert_eq!(strip_heading("####### Seven"), None);
}

#[test]
fn test_strip_heading_no_space() {
    assert_eq!(strip_heading("#NoSpace"), None);
}

#[test]
fn test_strip_heading_not_heading() {
    assert_eq!(strip_heading("regular text"), None);
}

// --- strip_ordered_list ---

#[test]
fn test_strip_ordered_list_basic() {
    assert_eq!(strip_ordered_list("1. First item"), Some("1. First item".to_string()));
}

#[test]
fn test_strip_ordered_list_double_digit() {
    assert_eq!(strip_ordered_list("10. Tenth"), Some("10. Tenth".to_string()));
}

#[test]
fn test_strip_ordered_list_not_list() {
    assert_eq!(strip_ordered_list("not a list"), None);
}

#[test]
fn test_strip_ordered_list_no_space() {
    assert_eq!(strip_ordered_list("1.no space"), None);
}

// --- convert_inline ---

#[test]
fn test_convert_inline_code() {
    let result = convert_inline("use `code` here");
    assert_eq!(result, "use <code>code</code> here");
}

#[test]
fn test_convert_inline_bold() {
    let result = convert_inline("some **bold** text");
    assert_eq!(result, "some <b>bold</b> text");
}

#[test]
fn test_convert_inline_italic() {
    let result = convert_inline("some *italic* text");
    assert_eq!(result, "some <i>italic</i> text");
}

#[test]
fn test_convert_inline_strikethrough() {
    let result = convert_inline("some ~~deleted~~ text");
    assert_eq!(result, "some <s>deleted</s> text");
}

#[test]
fn test_convert_inline_no_formatting() {
    let result = convert_inline("plain text");
    assert_eq!(result, "plain text");
}

// --- convert_links ---

#[test]
fn test_convert_links_basic() {
    let result = convert_links("[click here](https://example.com)");
    assert_eq!(result, "<a href=\"https://example.com\">click here</a>");
}

#[test]
fn test_convert_links_no_link() {
    let result = convert_links("just text");
    assert_eq!(result, "just text");
}

#[test]
fn test_convert_links_empty_url() {
    let result = convert_links("[text]()");
    assert_eq!(result, "[text]()");
}

#[test]
fn test_convert_links_multiple() {
    let result = convert_links("[a](http://a.com) and [b](http://b.com)");
    assert!(result.contains("<a href=\"http://a.com\">a</a>"));
    assert!(result.contains("<a href=\"http://b.com\">b</a>"));
}

// --- convert_bold_italic_strike ---

#[test]
fn test_bold_italic_strike_bold() {
    assert_eq!(convert_bold_italic_strike("**hello**"), "<b>hello</b>");
}

#[test]
fn test_bold_italic_strike_italic() {
    assert_eq!(convert_bold_italic_strike("*hello*"), "<i>hello</i>");
}

#[test]
fn test_bold_italic_strike_strike() {
    assert_eq!(convert_bold_italic_strike("~~hello~~"), "<s>hello</s>");
}

#[test]
fn test_bold_italic_strike_mixed() {
    let result = convert_bold_italic_strike("**bold** and *italic*");
    assert!(result.contains("<b>bold</b>"));
    assert!(result.contains("<i>italic</i>"));
}

#[test]
fn test_bold_italic_strike_unclosed() {
    // Unclosed markers should be left as-is
    assert_eq!(convert_bold_italic_strike("**unclosed"), "**unclosed");
}

// --- markdown_to_telegram_html ---

#[test]
fn test_md_to_html_plain_text() {
    let result = markdown_to_telegram_html("Hello world");
    assert_eq!(result, "Hello world");
}

#[test]
fn test_md_to_html_code_block() {
    let result = markdown_to_telegram_html("```rust\nfn main() {}\n```");
    assert!(result.contains("<pre>"));
    assert!(result.contains("language-rust"));
    assert!(result.contains("fn main() {}"));
}

#[test]
fn test_md_to_html_code_block_no_lang() {
    let result = markdown_to_telegram_html("```\nsome code\n```");
    assert!(result.contains("<pre>"));
    assert!(result.contains("some code"));
    assert!(!result.contains("language-"));
}

#[test]
fn test_md_to_html_heading() {
    let result = markdown_to_telegram_html("# Title");
    assert!(result.contains("<b>Title</b>"));
}

#[test]
fn test_md_to_html_horizontal_rule() {
    let result = markdown_to_telegram_html("---");
    assert!(result.contains("———————————"));
}

#[test]
fn test_md_to_html_blockquote() {
    let result = markdown_to_telegram_html("> quoted text");
    assert!(result.contains("┃"));
    assert!(result.contains("<i>quoted text</i>"));
}

#[test]
fn test_md_to_html_unordered_list_dash() {
    let result = markdown_to_telegram_html("- item one");
    assert!(result.contains("• item one"));
}

#[test]
fn test_md_to_html_unordered_list_star() {
    let result = markdown_to_telegram_html("* item one");
    assert!(result.contains("• item one"));
}

#[test]
fn test_md_to_html_inline_bold() {
    let result = markdown_to_telegram_html("this is **bold** text");
    assert!(result.contains("<b>bold</b>"));
}

#[test]
fn test_md_to_html_html_entities_escaped() {
    let result = markdown_to_telegram_html("a < b & c > d");
    assert!(result.contains("&lt;"));
    assert!(result.contains("&amp;"));
    assert!(result.contains("&gt;"));
}

#[test]
fn test_md_to_html_complex() {
    let md = "# Hello\n\nSome **bold** text.\n\n```rust\nfn main() {}\n```\n\n- item 1\n- item 2";
    let result = markdown_to_telegram_html(md);
    assert!(result.contains("<b>Hello</b>"));
    assert!(result.contains("<b>bold</b>"));
    assert!(result.contains("<pre>"));
    assert!(result.contains("• item 1"));
    assert!(result.contains("• item 2"));
}

// --- normalize_empty_lines ---

#[test]
fn test_normalize_empty_lines_collapses() {
    assert_eq!(normalize_empty_lines("a\n\n\n\nb"), "a\n\nb");
}

#[test]
fn test_normalize_empty_lines_single_blank() {
    assert_eq!(normalize_empty_lines("a\n\nb"), "a\n\nb");
}

#[test]
fn test_normalize_empty_lines_no_blanks() {
    assert_eq!(normalize_empty_lines("a\nb"), "a\nb");
}

// --- truncate_str ---

#[test]
fn test_truncate_str_short() {
    assert_eq!(truncate_str("hello", 100), "hello");
}

#[test]
fn test_truncate_str_at_newline() {
    let s = "line1\nline2\nline3";
    let result = truncate_str(s, 10);
    assert_eq!(result, "line1");
}

#[test]
fn test_truncate_str_no_newline() {
    let s = "abcdefghijklmnop";
    let result = truncate_str(s, 5);
    assert_eq!(result, "abcde");
}

// --- format_tool_input ---

#[test]
fn test_format_tool_input_bash() {
    let input = r#"{"command":"ls -la","description":"List files"}"#;
    let result = crate::services::formatter::format_tool_input("Bash", input, false);
    assert_eq!(result, "List files: `ls -la`");
}

#[test]
fn test_format_tool_input_bash_no_desc() {
    let input = r#"{"command":"pwd"}"#;
    let result = crate::services::formatter::format_tool_input("Bash", input, false);
    assert_eq!(result, "`pwd`");
}

#[test]
fn test_format_tool_input_read() {
    let input = r#"{"file_path":"/src/main.rs"}"#;
    let result = crate::services::formatter::format_tool_input("Read", input, false);
    assert_eq!(result, "Read /src/main.rs");
}

#[test]
fn test_format_tool_input_write() {
    let input = r#"{"file_path":"/test.rs","content":"line1\nline2\nline3"}"#;
    let result = crate::services::formatter::format_tool_input("Write", input, false);
    assert!(result.starts_with("Write /test.rs"));
}

#[test]
fn test_format_tool_input_glob() {
    let input = r#"{"pattern":"*.rs","path":"/src"}"#;
    let result = crate::services::formatter::format_tool_input("Glob", input, false);
    assert_eq!(result, "Glob *.rs in /src");
}

#[test]
fn test_format_tool_input_grep() {
    let input = r#"{"pattern":"fn main","path":"/src"}"#;
    let result = crate::services::formatter::format_tool_input("Grep", input, false);
    assert_eq!(result, "Grep \"fn main\" in /src");
}

#[test]
fn test_format_tool_input_websearch() {
    let input = r#"{"query":"rust async"}"#;
    let result = crate::services::formatter::format_tool_input("WebSearch", input, false);
    assert_eq!(result, "Search: rust async");
}

#[test]
fn test_format_tool_input_task() {
    let input = r#"{"description":"Explore codebase","subagent_type":"Explore"}"#;
    let result = crate::services::formatter::format_tool_input("Task", input, false);
    assert_eq!(result, "Task [Explore]: Explore codebase");
}

#[test]
fn test_format_tool_input_unknown_tool() {
    let input = "some raw text";
    let result = crate::services::formatter::format_tool_input("CustomTool", input, false);
    assert!(result.starts_with("CustomTool "));
}

#[test]
fn test_format_tool_input_invalid_json() {
    let result = crate::services::formatter::format_tool_input("Bash", "not json", false);
    assert!(result.contains("Bash"));
}

// --- markdown_to_telegram_html edge cases ---

#[test]
fn test_md_to_html_nested_bold_in_list() {
    let result = markdown_to_telegram_html("- **bold** item");
    assert!(result.contains("• <b>bold</b> item"));
}

#[test]
fn test_md_to_html_link() {
    let result = markdown_to_telegram_html("[click](https://example.com)");
    assert!(result.contains("<a href=\"https://example.com\">click</a>"));
}

#[test]
fn test_md_to_html_blockquote_empty() {
    let result = markdown_to_telegram_html(">");
    assert!(result.contains("┃"));
}

#[test]
fn test_md_to_html_ordered_list() {
    let result = markdown_to_telegram_html("1. First\n2. Second");
    assert!(result.contains("1. First"));
    assert!(result.contains("2. Second"));
}

// --- convert_links_and_formatting ---

#[test]
fn test_convert_links_and_formatting_combined() {
    let result = convert_links_and_formatting("[link](http://a.com) and **bold**");
    assert!(result.contains("<a href=\"http://a.com\">link</a>"));
    assert!(result.contains("<b>bold</b>"));
}

// --- find_closing_marker edge cases ---

#[test]
fn test_find_closing_marker_empty_content() {
    let chars: Vec<char> = "****".chars().collect();
    // start=2, looking for ** at position 2 → empty content, should return None
    assert_eq!(find_closing_marker(&chars, 2, &['*', '*']), None);
}

#[test]
fn test_find_closing_single_at_boundary() {
    let chars: Vec<char> = "*hello*".chars().collect();
    assert_eq!(find_closing_single(&chars, 1, '*'), Some(6));
}

#[test]
fn test_find_closing_single_double_marker_skipped() {
    // **bold** — looking for single * from pos 1 should skip the ** at pos 5-6
    let chars: Vec<char> = "**bold**".chars().collect();
    assert_eq!(find_closing_single(&chars, 1, '*'), None);
}

// --- floor_char_boundary (telegram version) ---

#[test]
fn test_floor_char_boundary_basic() {
    assert_eq!(floor_char_boundary("hello", 3), 3);
}

#[test]
fn test_floor_char_boundary_beyond_end() {
    assert_eq!(floor_char_boundary("hi", 10), 2);
}

#[test]
fn test_floor_char_boundary_multibyte() {
    let s = "가나다"; // each 3 bytes
    assert_eq!(floor_char_boundary(s, 4), 3);
}
