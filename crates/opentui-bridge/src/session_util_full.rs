//! Session list labels (std-only).
#![forbid(unsafe_code)]

/// Display title: trimmed title or first 8 of id, capped at 128 chars.
#[must_use]
pub fn session_title(title: &str, id: &str) -> String {
    let t = title.trim();
    let s = if t.is_empty() {
        id.chars().take(8).collect::<String>()
    } else {
        t.to_string()
    };
    if s.chars().count() > 128 {
        s.chars().take(128).collect()
    } else {
        s
    }
}

/// Age bucket label: Xs | Xm | Xh | Xd (floor).
#[must_use]
pub fn age_label(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3_600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h", secs / 3_600)
    } else {
        format!("{}d", secs / 86_400)
    }
}

/// Message count label: "N msgs".
#[must_use]
pub fn msg_count_label(n: usize) -> String {
    format!("{n} msgs")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_passthrough() {
        assert_eq!(session_title("Hello", "1234567890"), "Hello");
    }

    #[test]
    fn title_empty_falls_back_to_id8() {
        assert_eq!(session_title("", "1234567890"), "12345678");
        assert_eq!(session_title("   ", "abcdef"), "abcdef");
    }

    #[test]
    fn title_capped_at_128_chars() {
        let long = "x".repeat(200);
        let out = session_title(&long, "id");
        assert_eq!(out.chars().count(), 128);
    }

    #[test]
    fn age_buckets() {
        assert_eq!(age_label(5), "5s");
        assert_eq!(age_label(59), "59s");
        assert_eq!(age_label(60), "1m");
        assert_eq!(age_label(3_599), "59m");
        assert_eq!(age_label(3_600), "1h");
        assert_eq!(age_label(86_399), "23h");
        assert_eq!(age_label(86_400), "1d");
        assert_eq!(age_label(172_800), "2d");
    }

    #[test]
    fn msg_count() {
        assert_eq!(msg_count_label(0), "0 msgs");
        assert_eq!(msg_count_label(7), "7 msgs");
    }
}
