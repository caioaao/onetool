/// Truncates output string to a maximum length, appending a truncation notice if needed.
///
/// If the input string is shorter than `max_length`, it is returned unchanged.
/// Otherwise, the string is truncated to exactly `max_length` characters, with the
/// suffix "...\n(output truncated)" included in the truncated result.
///
/// # Panics
///
/// Panics if `max_length` is less than or equal to 22 (the length of the truncation suffix).
/// This is considered a programming error - `max_length` must be large enough to accommodate
/// at least the suffix plus one character from the original string.
pub fn truncate_output(s: &str, max_length: usize) -> String {
    const SUFFIX: &str = "...\n(output truncated)";
    const SUFFIX_LEN: usize = 22;

    // Count characters in the input string
    let char_count = s.chars().count();

    if char_count <= max_length {
        return s.to_string();
    }

    // Truncation is needed - ensure max_length is large enough for suffix
    if max_length <= SUFFIX_LEN {
        panic!(
            "max_length must be greater than {} (the suffix length) when truncation is needed, got {}",
            SUFFIX_LEN, max_length
        );
    }

    // Calculate how many characters from the original string to include
    let prefix_char_count = max_length - SUFFIX_LEN;

    // Take exactly prefix_char_count characters
    let prefix: String = s.chars().take(prefix_char_count).collect();

    format!("{}{}", prefix, SUFFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_truncation_needed_short_string() {
        let input = "hello";
        let result = truncate_output(input, 100);
        assert_eq!(result, "hello");
        assert_eq!(result.chars().count(), 5);
    }

    #[test]
    fn test_no_truncation_exact_length() {
        let input = "hello";
        let result = truncate_output(input, 5);
        assert_eq!(result, "hello");
        assert_eq!(result.chars().count(), 5);
    }

    #[test]
    fn test_basic_truncation() {
        let input = "hello world this is a long string that needs truncation";
        let result = truncate_output(input, 30);
        assert_eq!(
            result.chars().count(),
            30,
            "Result should have exactly 30 characters"
        );
        assert!(result.ends_with("...\n(output truncated)"));
        // Should have 8 chars from original (30 - 22)
        assert_eq!(&result[..8], "hello wo");
    }

    #[test]
    fn test_exact_length_verification() {
        let input = "a".repeat(1000);
        let max_len = 50;
        let result = truncate_output(&input, max_len);
        assert_eq!(
            result.chars().count(),
            max_len,
            "Result character count must equal max_length"
        );
    }

    #[test]
    #[should_panic(expected = "max_length must be greater than 22")]
    fn test_panic_on_insufficient_max_length() {
        // Long string that needs truncation, but max_length too small
        let input = "this is a very long string that definitely needs truncation";
        truncate_output(input, 22);
    }

    #[test]
    #[should_panic(expected = "max_length must be greater than 22")]
    fn test_panic_on_zero_max_length() {
        // Long string that needs truncation, but max_length is 0
        let input = "this is a very long string that definitely needs truncation";
        truncate_output(input, 0);
    }

    #[test]
    fn test_empty_string() {
        let result = truncate_output("", 100);
        assert_eq!(result, "");
    }

    #[test]
    fn test_unicode_characters() {
        // Test with emoji (4-byte UTF-8 characters) - make it longer to trigger truncation
        let input = "Hello 👋 World 🌍 Test 🚀 More emoji 🎉 and text 🔥 continues here";
        let result = truncate_output(input, 30);
        assert_eq!(
            result.chars().count(),
            30,
            "Result should have exactly 30 characters"
        );
        assert!(result.ends_with("...\n(output truncated)"));
        // Verify the string is valid UTF-8 (all Rust strings are valid UTF-8)
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }

    #[test]
    fn test_minimum_valid_max_length() {
        // max_length = 23 should work (suffix 22 + 1 char from input)
        let input = "hello world this is a long string";
        let result = truncate_output(input, 23);
        assert_eq!(
            result.chars().count(),
            23,
            "Result should have exactly 23 characters"
        );
        assert_eq!(&result[..1], "h");
        assert!(result.ends_with("...\n(output truncated)"));
    }

    #[test]
    fn test_boundary_case_one_more_than_length() {
        // Input is 5 chars, max_length is 6, no truncation needed
        let input = "hello";
        let result = truncate_output(input, 6);
        assert_eq!(result, "hello");
        assert_eq!(result.chars().count(), 5);
    }

    #[test]
    fn test_multibyte_unicode_boundary() {
        // String with various multibyte characters (longer than truncation point)
        let input = "日本語テストですこんにちは世界またお会いしましょうさようなら"; // Japanese characters (30+ chars)
        // Truncate to 28 characters total = 6 chars from input + 22 char suffix
        let result = truncate_output(input, 28);
        assert_eq!(
            result.chars().count(),
            28,
            "Result should have exactly 28 characters"
        );
        assert!(result.ends_with("...\n(output truncated)"));
        // Should have first 6 characters from input: 日本語テスト
        assert!(result.starts_with("日本語テスト"));
        // Verify the string is valid UTF-8
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }

    #[test]
    fn test_truncation_with_newlines() {
        let input = "line1\nline2\nline3\nline4\nline5\nline6\nline7";
        let result = truncate_output(input, 35);
        assert_eq!(
            result.chars().count(),
            35,
            "Result should have exactly 35 characters"
        );
        assert!(result.ends_with("...\n(output truncated)"));
    }
}
