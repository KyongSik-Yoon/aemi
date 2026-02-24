use super::messages::html_escape;

/// Convert standard markdown to Telegram-compatible HTML
pub fn markdown_to_telegram_html(md: &str) -> String {
    let lines: Vec<&str> = md.lines().collect();
    let mut result = String::new();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim_start();

        // Fenced code block
        if trimmed.starts_with("```") {
            // Extract language hint from opening ``` (e.g., ```diff, ```rust)
            let lang = trimmed.trim_start_matches('`').trim();
            let mut code_lines = Vec::new();
            i += 1; // skip opening ```
            while i < lines.len() {
                if lines[i].trim_start().starts_with("```") {
                    break;
                }
                code_lines.push(lines[i]);
                i += 1;
            }
            let code = code_lines.join("\n");
            if !code.is_empty() {
                if !lang.is_empty() {
                    result.push_str(&format!(
                        "<pre><code class=\"language-{}\">{}</code></pre>",
                        html_escape(lang),
                        html_escape(code.trim_end())
                    ));
                } else {
                    result.push_str(&format!("<pre>{}</pre>", html_escape(code.trim_end())));
                }
            }
            result.push('\n');
            i += 1; // skip closing ```
            continue;
        }

        // Horizontal rule (---, ***, ___)
        if is_horizontal_rule(trimmed) {
            result.push_str("———————————\n");
            i += 1;
            continue;
        }

        // Heading (# ~ ######)
        if let Some(rest) = strip_heading(trimmed) {
            result.push_str(&format!("<b>{}</b>", convert_inline(&html_escape(rest))));
            result.push('\n');
            i += 1;
            continue;
        }

        // Blockquote (> text)
        if trimmed.starts_with("> ") || trimmed == ">" {
            let quote_content = if trimmed.len() > 2 { &trimmed[2..] } else { "" };
            result.push_str(&format!("┃ <i>{}</i>", convert_inline(&html_escape(quote_content))));
            result.push('\n');
            i += 1;
            continue;
        }

        // Ordered list (1. 2. 3.)
        if let Some(rest) = strip_ordered_list(trimmed) {
            result.push_str(&convert_inline(&html_escape(&rest)));
            result.push('\n');
            i += 1;
            continue;
        }

        // Unordered list (- or *)
        if trimmed.starts_with("- ") {
            result.push_str(&format!("• {}", convert_inline(&html_escape(&trimmed[2..]))));
            result.push('\n');
            i += 1;
            continue;
        }
        if trimmed.starts_with("* ") && !trimmed.starts_with("**") {
            result.push_str(&format!("• {}", convert_inline(&html_escape(&trimmed[2..]))));
            result.push('\n');
            i += 1;
            continue;
        }

        // Regular line
        result.push_str(&convert_inline(&html_escape(lines[i])));
        result.push('\n');
        i += 1;
    }

    result.trim_end().to_string()
}

/// Check if a line is a horizontal rule (---, ***, ___, or variants with spaces)
pub fn is_horizontal_rule(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 3 {
        return false;
    }
    let chars: Vec<char> = trimmed.chars().collect();
    let first = chars[0];
    if first != '-' && first != '*' && first != '_' {
        return false;
    }
    chars.iter().all(|&c| c == first || c == ' ')
        && chars.iter().filter(|&&c| c == first).count() >= 3
}

/// Strip ordered list prefix (e.g., "1. ", "2. ", "10. ") and return with number preserved
pub fn strip_ordered_list(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let mut i = 0;
    // Must start with digits
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    // Must have at least one digit, followed by ". "
    if i > 0 && i < bytes.len() - 1 && bytes[i] == b'.' && bytes[i + 1] == b' ' {
        let number = &line[..i];
        let rest = &line[i + 2..];
        Some(format!("{}. {}", number, rest))
    } else {
        None
    }
}

/// Strip markdown heading prefix (# ~ ######), return remaining text
pub fn strip_heading(line: &str) -> Option<&str> {
    let trimmed = line.trim_start_matches('#');
    // Must have consumed at least one # and be followed by a space
    if trimmed.len() < line.len() && trimmed.starts_with(' ') {
        let hashes = line.len() - trimmed.len();
        if hashes <= 6 {
            return Some(trimmed.trim_start());
        }
    }
    None
}

/// Convert inline markdown elements (bold, italic, code, links, strikethrough) in already HTML-escaped text
pub fn convert_inline(text: &str) -> String {
    // Process inline code first to protect content from further conversion
    let mut result = String::new();
    let mut remaining = text;

    // Split by inline code spans: `...`
    loop {
        if let Some(start) = remaining.find('`') {
            let after_start = &remaining[start + 1..];
            if let Some(end) = after_start.find('`') {
                // Found a complete inline code span
                let before = &remaining[..start];
                let code_content = &after_start[..end];
                result.push_str(&convert_links_and_formatting(before));
                result.push_str(&format!("<code>{}</code>", code_content));
                remaining = &after_start[end + 1..];
                continue;
            }
        }
        // No more inline code spans
        result.push_str(&convert_links_and_formatting(remaining));
        break;
    }

    result
}

/// Convert markdown links [text](url), then bold/italic/strikethrough
pub fn convert_links_and_formatting(text: &str) -> String {
    // First convert links, then apply bold/italic/strikethrough to the result
    let linked = convert_links(text);
    convert_bold_italic_strike(&linked)
}

/// Convert markdown links [text](url) to Telegram HTML <a> tags
/// Input text is already HTML-escaped, so we look for escaped brackets
pub fn convert_links(text: &str) -> String {
    let mut result = String::new();
    let mut remaining = text;

    loop {
        // Find [text](url) pattern
        if let Some(bracket_start) = remaining.find('[') {
            let after_bracket = &remaining[bracket_start + 1..];
            if let Some(bracket_end) = after_bracket.find("](") {
                let link_text = &after_bracket[..bracket_end];
                let after_paren = &after_bracket[bracket_end + 2..];
                if let Some(paren_end) = after_paren.find(')') {
                    let url = &after_paren[..paren_end];
                    // Don't convert if URL is empty or link_text is empty
                    if !url.is_empty() && !link_text.is_empty() {
                        result.push_str(&remaining[..bracket_start]);
                        result.push_str(&format!("<a href=\"{}\">{}</a>", url, link_text));
                        remaining = &after_paren[paren_end + 1..];
                        continue;
                    }
                }
            }
            // Not a valid link, output the [ and continue
            result.push_str(&remaining[..bracket_start + 1]);
            remaining = &remaining[bracket_start + 1..];
            continue;
        }
        result.push_str(remaining);
        break;
    }

    result
}

/// Convert bold (**...**), italic (*...*), and strikethrough (~~...~~) in text
pub fn convert_bold_italic_strike(text: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Strikethrough: ~~...~~
        if i + 1 < len && chars[i] == '~' && chars[i + 1] == '~' {
            if let Some(end) = find_closing_marker(&chars, i + 2, &['~', '~']) {
                let inner: String = chars[i + 2..end].iter().collect();
                result.push_str(&format!("<s>{}</s>", inner));
                i = end + 2;
                continue;
            }
        }
        // Bold: **...**
        if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
            if let Some(end) = find_closing_marker(&chars, i + 2, &['*', '*']) {
                let inner: String = chars[i + 2..end].iter().collect();
                result.push_str(&format!("<b>{}</b>", inner));
                i = end + 2;
                continue;
            }
        }
        // Italic: *...*
        if chars[i] == '*' {
            if let Some(end) = find_closing_single(&chars, i + 1, '*') {
                let inner: String = chars[i + 1..end].iter().collect();
                result.push_str(&format!("<i>{}</i>", inner));
                i = end + 1;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }

    result
}

/// Find closing double marker (e.g., **) starting from pos
pub fn find_closing_marker(chars: &[char], start: usize, marker: &[char; 2]) -> Option<usize> {
    let len = chars.len();
    let mut i = start;
    while i + 1 < len {
        if chars[i] == marker[0] && chars[i + 1] == marker[1] {
            // Don't match empty content
            if i > start {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Find closing single marker (e.g., *) starting from pos
pub fn find_closing_single(chars: &[char], start: usize, marker: char) -> Option<usize> {
    let len = chars.len();
    let mut i = start;
    while i < len {
        if chars[i] == marker {
            // Skip double markers entirely
            if i + 1 < len && chars[i + 1] == marker {
                i += 2;
                continue;
            }
            // Don't match empty
            if i > start {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}
