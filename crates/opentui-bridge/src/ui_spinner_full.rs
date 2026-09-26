#![forbid(unsafe_code)]
//! Minimal braille spinner (first 4 of `SPINNER_FRAMES`, `spinner.tsx:10`).
//! Sibling `spinner_full.rs:6` covers first 8; `spinner.rs` covers the
//! Knight Rider scanner from `ui/spinner.ts:272-329` (no 4-frame const).

/// First 4 of TS `SPINNER_FRAMES` (`component/spinner.tsx:10`).
pub const SPINNER_FRAMES: [&str; 4] = ["⠋", "⠙", "⠹", "⠸"];

/// Frame at index, wrapping (`i % len`).
#[must_use]
pub fn frame_at(i: usize) -> &'static str {
    SPINNER_FRAMES[i % SPINNER_FRAMES.len()]
}

/// Frame count (4).
#[must_use]
pub const fn frame_count() -> usize {
    SPINNER_FRAMES.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_match_ts() {
        assert_eq!(SPINNER_FRAMES, ["⠋", "⠙", "⠹", "⠸"]);
    }

    #[test]
    fn frame_at_wraps() {
        assert_eq!(frame_at(0), "⠋");
        assert_eq!(frame_at(4), frame_at(0));
        assert_eq!(frame_at(9), SPINNER_FRAMES[1]);
    }

    #[test]
    fn count_is_four() {
        assert_eq!(frame_count(), 4);
        assert_eq!(frame_count(), SPINNER_FRAMES.len());
    }
}
