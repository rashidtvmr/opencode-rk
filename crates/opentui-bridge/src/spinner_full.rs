#![forbid(unsafe_code)]
//! Braille spinner (mirrors `spinner.tsx:10` SPINNER_FRAMES, first 8).
//! Sibling `spinner.rs` covers knight-rider scanner; this covers braille dots.

/// First 8 of TS `SPINNER_FRAMES` (`spinner.tsx:10`).
pub const FRAMES: [&'static str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

/// Cyclic braille spinner cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Spinner {
    pub frame: usize,
}

impl Spinner {
    #[must_use]
    pub const fn new() -> Self {
        Self { frame: 0 }
    }

    /// Advance one frame, wrapping at 8.
    pub fn tick(&mut self) {
        self.frame = (self.frame + 1) % FRAMES.len();
    }

    /// Current glyph.
    #[must_use]
    pub fn frame_of(&self) -> &str {
        FRAMES[self.frame % FRAMES.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_len_eight() {
        assert_eq!(FRAMES.len(), 8);
        assert_eq!(FRAMES[0], "⠋");
    }

    #[test]
    fn new_starts_zero() {
        let s = Spinner::new();
        assert_eq!(s.frame_of(), "⠋");
    }

    #[test]
    fn tick_wraps() {
        let mut s = Spinner::new();
        for _ in 0..7 {
            s.tick();
        }
        assert_eq!(s.frame_of(), "⠧");
        s.tick();
        assert_eq!(s.frame, 0);
    }

    #[test]
    fn sequence_matches_ts() {
        let mut s = Spinner::new();
        let mut got: Vec<String> = Vec::new();
        for _ in 0..8 {
            got.push(s.frame_of().to_string());
            s.tick();
        }
        let want: Vec<String> = FRAMES.iter().map(|f| f.to_string()).collect();
        assert_eq!(got, want);
    }

    #[test]
    fn large_frame_safe() {
        let s = Spinner { frame: usize::MAX };
        assert_eq!(s.frame_of(), FRAMES[usize::MAX % 8]);
    }
}
