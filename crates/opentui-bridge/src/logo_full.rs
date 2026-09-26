#![forbid(unsafe_code)]
//! Full ASCII logo banner (mirrors `logo.ts` wordmark, plain ASCII only).

/// Banner rows, at most 3.
pub const LOGO_ROWS: [&str; 3] = [
    " ___  ___  ___ _ __   ___ ___  ___ ___ ",
    "| _ || _ || __|| '_ | / __/ __|| _ | __|",
    "|___|| .__||___||_| |_|___/___||___||___|",
];

/// Brand text.
#[must_use]
pub fn logo_text() -> &'static str {
    "opencode"
}

/// Owned banner rows (capped at 3).
#[must_use]
pub fn logo_lines() -> Vec<String> {
    LOGO_ROWS.iter().map(|s| s.to_string()).take(3).collect()
}

/// Max row char count.
#[must_use]
pub fn logo_width() -> usize {
    LOGO_ROWS
        .iter()
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_opencode() {
        assert_eq!(logo_text(), "opencode");
    }

    #[test]
    fn lines_capped_ascii() {
        let l = logo_lines();
        assert!(!l.is_empty() && l.len() <= 3);
        assert!(l.iter().all(|s| s.is_ascii()));
    }

    #[test]
    fn width_matches_max() {
        let w = logo_width();
        let l = logo_lines();
        assert_eq!(w, l.iter().map(|s| s.chars().count()).max().unwrap());
    }

    #[test]
    fn rows_match_const() {
        assert_eq!(
            logo_lines(),
            LOGO_ROWS.iter().map(|s| s.to_string()).collect::<Vec<_>>()
        );
    }
}
