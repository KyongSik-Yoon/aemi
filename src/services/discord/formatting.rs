use crate::services::formatter;

/// Sanitize stray triple-backtick sequences that appear outside code blocks.
/// Discord interprets any ``` as a code fence marker, so inline occurrences
/// (e.g. "use ```rust for code blocks") break rendering. This inserts a
/// zero-width space to break the sequence: ``` → `\u{200B}``
pub fn sanitize_inline_backticks(text: &str) -> String {
    let mut result = String::with_capacity(text.len() + 64);
    let mut in_code_block = false;
    let mut open_fence_len: usize = 0;

    for (i, line) in text.lines().enumerate() {
        if i > 0 {
            result.push('\n');
        }

        let trimmed = line.trim_start();
        let bt_count = if trimmed.starts_with('`') {
            trimmed.bytes().take_while(|&b| b == b'`').count()
        } else {
            0
        };

        if in_code_block {
            // Check for closing fence (>= opening length, rest is whitespace)
            if bt_count >= open_fence_len && trimmed[bt_count..].trim().is_empty() {
                in_code_block = false;
            }
            result.push_str(line);
        } else if bt_count >= 3 {
            // Potential opening fence — valid if remainder has no backticks
            let after_bt = &trimmed[bt_count..];
            if !after_bt.contains('`') {
                in_code_block = true;
                open_fence_len = bt_count;
                result.push_str(line);
            } else {
                // Starts with ``` but has more backticks later — escape
                result.push_str(&line.replace("```", "`\u{200B}``"));
            }
        } else if line.contains("```") {
            // Inline ``` not at line start — escape
            result.push_str(&line.replace("```", "`\u{200B}``"));
        } else {
            result.push_str(line);
        }
    }

    result
}

/// Fix code blocks that contain diff content but use a non-diff language hint.
/// Discord only applies +/- coloring (green/red) when the code block uses ```diff.
/// Claude sometimes wraps diff-like content in ```kotlin, ```rust, etc.
pub fn fix_diff_code_blocks(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        // Look for code fence opening: ``` at start of line (possibly after whitespace)
        if let Some(fence_start) = find_code_fence(bytes, pos) {
            let fence_line_end = memchr_newline(bytes, fence_start);
            let backticks_end = fence_start + count_backticks(bytes, fence_start);
            let lang_hint = text[backticks_end..fence_line_end].trim();
            let backtick_count = backticks_end - fence_start;

            // Find the closing fence
            if let Some(close_start) = find_closing_fence(bytes, fence_line_end, backtick_count) {
                let block_content = &text[fence_line_end..close_start];
                let block_content = block_content.strip_prefix('\n').unwrap_or(block_content);
                let close_line_end = memchr_newline(bytes, close_start);

                // Only fix blocks with a non-diff language hint that contain diff content
                if !lang_hint.is_empty() && lang_hint != "diff" && formatter::is_diff_content(block_content) {
                    // Copy everything before this fence
                    result.push_str(&text[pos..fence_start]);
                    // Write corrected fence with diff hint + newline
                    for _ in 0..backtick_count {
                        result.push('`');
                    }
                    result.push_str("diff\n");
                    // Copy from content start to end of closing fence line
                    result.push_str(&text[fence_line_end..close_line_end]);
                    pos = close_line_end;
                } else {
                    // No change needed, copy through closing fence
                    result.push_str(&text[pos..close_line_end]);
                    pos = close_line_end;
                }
            } else {
                // No closing fence found, copy the fence line as-is
                result.push_str(&text[pos..fence_line_end]);
                pos = fence_line_end;
            }
        } else {
            // No more code fences, copy rest of text
            result.push_str(&text[pos..]);
            break;
        }
    }

    result
}

/// Find the next code fence (```) starting from `from`, at start of a line.
pub fn find_code_fence(bytes: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i < bytes.len() {
        // Must be at start of line (i==0 or preceded by \n)
        if i == 0 || bytes[i - 1] == b'\n' {
            // Skip optional leading whitespace
            let mut j = i;
            while j < bytes.len() && bytes[j] == b' ' {
                j += 1;
            }
            if j + 2 < bytes.len() && bytes[j] == b'`' && bytes[j + 1] == b'`' && bytes[j + 2] == b'`' {
                return Some(j);
            }
        }
        // Advance to next line
        match bytes[i..].iter().position(|&b| b == b'\n') {
            Some(nl) => i += nl + 1,
            None => break,
        }
    }
    None
}

pub fn count_backticks(bytes: &[u8], from: usize) -> usize {
    let mut count = 0;
    while from + count < bytes.len() && bytes[from + count] == b'`' {
        count += 1;
    }
    count
}

pub fn memchr_newline(bytes: &[u8], from: usize) -> usize {
    match bytes[from..].iter().position(|&b| b == b'\n') {
        Some(nl) => from + nl + 1,
        None => bytes.len(),
    }
}

pub fn find_closing_fence(bytes: &[u8], from: usize, backtick_count: usize) -> Option<usize> {
    let mut i = from;
    // Skip past opening line
    if i < bytes.len() && bytes[i] == b'\n' {
        i += 1;
    }
    while i < bytes.len() {
        // Must be at start of line
        let line_start = i;
        // Skip optional leading whitespace
        while i < bytes.len() && bytes[i] == b' ' {
            i += 1;
        }
        // Check for backticks
        let bt_start = i;
        let bt_count = count_backticks(bytes, i);
        if bt_count >= backtick_count {
            // Check rest of line is empty (or just whitespace)
            let rest_start = bt_start + bt_count;
            let line_end = bytes[rest_start..].iter().position(|&b| b == b'\n')
                .map(|p| rest_start + p)
                .unwrap_or(bytes.len());
            let rest = &bytes[rest_start..line_end];
            if rest.iter().all(|&b| b == b' ' || b == b'\t') {
                return Some(line_start);
            }
        }
        // Advance to next line
        match bytes[i..].iter().position(|&b| b == b'\n') {
            Some(nl) => i += nl + 1,
            None => break,
        }
    }
    None
}
