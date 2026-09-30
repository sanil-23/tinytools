//! Small text helpers shared by the filesystem tools.

/// Truncate `s` to at most `max_bytes` bytes, appending a single-character
/// ellipsis `…` (3 bytes) if truncated. The returned string's total byte
/// length never exceeds `max_bytes`.
pub(super) fn truncate_at_byte_boundary(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let ellipsis = "…";
    let ellipsis_len = ellipsis.len();
    if max_bytes < ellipsis_len {
        return String::new();
    }
    let mut end = max_bytes - ellipsis_len;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{}", &s[..end], ellipsis)
}

#[cfg(test)]
mod test {
    use super::truncate_at_byte_boundary;

    #[test]
    fn short_strings_pass_through() {
        assert_eq!(truncate_at_byte_boundary("abc", 3), "abc");
    }

    #[test]
    fn long_strings_end_in_an_ellipsis_within_budget() {
        let out = truncate_at_byte_boundary("abcdefghij", 6);
        assert_eq!(out, "abc…");
        assert!(out.len() <= 6);
    }

    #[test]
    fn never_splits_a_multibyte_character() {
        // 'é' is two bytes; a budget landing mid-character backs up.
        let out = truncate_at_byte_boundary("ééééé", 6);
        assert_eq!(out, "é…");
    }

    #[test]
    fn a_budget_smaller_than_the_ellipsis_yields_nothing() {
        assert_eq!(truncate_at_byte_boundary("abcdef", 2), "");
    }
}
