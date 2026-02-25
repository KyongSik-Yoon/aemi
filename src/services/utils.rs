/// Shared utility functions used across multiple modules.

/// Macro to generate the common boilerplate for AI service modules:
/// - A `OnceLock`-cached path resolver (`resolve_*_path`, `get_*_path`)
/// - A `debug_log` function that writes to `~/.aemi/debug/<name>.log`
///
/// Usage: `define_ai_service_helpers!("claude");`
#[macro_export]
macro_rules! define_ai_service_helpers {
    ($binary_name:expr) => {
        use std::sync::OnceLock;
        use std::process::Command;
        use std::io::Write;
        use std::fs::OpenOptions;

        static BINARY_PATH: OnceLock<Option<String>> = OnceLock::new();

        fn resolve_binary_path() -> Option<String> {
            // Try direct `which` first
            if let Ok(output) = Command::new("which").arg($binary_name).output() {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() {
                        return Some(path);
                    }
                }
            }
            // Fallback: use login shell to resolve PATH
            if let Ok(output) = Command::new("bash")
                .args(["-lc", &format!("which {}", $binary_name)])
                .output()
            {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() {
                        return Some(path);
                    }
                }
            }
            None
        }

        fn get_binary_path() -> Option<&'static str> {
            BINARY_PATH.get_or_init(|| resolve_binary_path()).as_deref()
        }

        fn debug_log(msg: &str) {
            static ENABLED: OnceLock<bool> = OnceLock::new();
            let enabled = ENABLED.get_or_init(|| {
                std::env::var("AEMI_DEBUG").map(|v| v == "1").unwrap_or(false)
            });
            if !*enabled { return; }
            if let Some(home) = dirs::home_dir() {
                let debug_dir = home.join(".aemi").join("debug");
                let _ = std::fs::create_dir_all(&debug_dir);
                let log_path = debug_dir.join(format!("{}.log", $binary_name));
                if let Ok(mut file) = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(log_path)
                {
                    let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
                    let _ = writeln!(file, "[{}] {}", timestamp, msg);
                }
            }
        }
    };
}

/// Find the nearest char boundary at or before the given byte index.
/// Returns `s.len()` if `index >= s.len()`.
pub fn floor_char_boundary(s: &str, index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    let mut i = index;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Truncate a string to `max_len` bytes, cutting at a safe UTF-8 char and line boundary.
/// If the string is shorter than `max_len`, returns the original string unchanged.
pub fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }

    let safe_end = floor_char_boundary(s, max_len);
    let truncated = &s[..safe_end];
    if let Some(pos) = truncated.rfind('\n') {
        truncated[..pos].to_string()
    } else {
        truncated.to_string()
    }
}

/// Normalize consecutive empty lines to a maximum of one blank line.
pub fn normalize_empty_lines(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut prev_was_empty = false;

    for line in s.lines() {
        let is_empty = line.is_empty();
        if is_empty {
            if !prev_was_empty {
                result.push('\n');
            }
            prev_was_empty = true;
        } else {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(line);
            prev_was_empty = false;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- floor_char_boundary ---

    #[test]
    fn test_floor_char_boundary_ascii() {
        assert_eq!(floor_char_boundary("hello", 3), 3);
    }

    #[test]
    fn test_floor_char_boundary_at_end() {
        assert_eq!(floor_char_boundary("hello", 10), 5);
    }

    #[test]
    fn test_floor_char_boundary_multibyte() {
        let s = "한글test"; // '한' = 3 bytes, '글' = 3 bytes
        assert_eq!(floor_char_boundary(s, 1), 0);
        assert_eq!(floor_char_boundary(s, 3), 3);
        assert_eq!(floor_char_boundary(s, 4), 3);
    }

    #[test]
    fn test_floor_char_boundary_empty() {
        assert_eq!(floor_char_boundary("", 0), 0);
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

    #[test]
    fn test_truncate_str_exact() {
        assert_eq!(truncate_str("hello", 5), "hello");
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

    #[test]
    fn test_normalize_no_blanks() {
        let input = "a\nb\nc";
        let result = normalize_empty_lines(input);
        assert_eq!(result, "a\nb\nc");
    }
}
