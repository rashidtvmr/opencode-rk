#![forbid(unsafe_code)]
//! Turn lifecycle wire over [`RuntimeSharedFull`] (BRIDGE-PAR-187).
//!
//! Thin adapter: `begin`/`finish` delegate to start/end turn,
//! `turn_line` renders `"model turns=N busy?"` capped at 256 chars.

use crate::runtime_shared_full::RuntimeSharedFull;

/// Max chars for [`TurnWire::turn_line`].
pub const TURN_LINE_CAP: usize = 256;

/// Turn wire wrapping shared runtime state.
#[derive(Debug, Default)]
pub struct TurnWire {
    pub rt: RuntimeSharedFull,
}

impl TurnWire {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin a turn; false when already busy.
    pub fn begin(&mut self) -> bool {
        self.rt.start_turn()
    }

    /// End a turn; bumps count saturating, clears busy.
    pub fn finish(&mut self) {
        self.rt.end_turn();
    }

    /// `"model turns=N busy?"` capped at 256 chars.
    #[must_use]
    pub fn turn_line(&self) -> String {
        let s = format!(
            "{} turns={} {}",
            self.rt.model(),
            self.rt.turns(),
            self.rt.is_busy()
        );
        if s.len() <= TURN_LINE_CAP {
            return s;
        }
        let mut out = String::with_capacity(TURN_LINE_CAP);
        for c in s.chars().take(TURN_LINE_CAP) {
            out.push(c);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_finish_cycle() {
        let mut w = TurnWire::new();
        assert!(w.begin());
        assert!(!w.begin());
        w.finish();
        assert!(w.rt.turns() == 1 && !w.rt.is_busy());
    }

    #[test]
    fn turn_line_format() {
        let mut w = TurnWire::new();
        w.rt.set_model("gpt");
        w.begin();
        assert_eq!(w.turn_line(), "gpt turns=0 true");
        w.finish();
        assert_eq!(w.turn_line(), "gpt turns=1 false");
    }

    #[test]
    fn turn_line_empty_model() {
        let w = TurnWire::new();
        assert_eq!(w.turn_line(), " turns=0 false");
    }

    #[test]
    fn turn_line_caps_256() {
        let mut w = TurnWire::new();
        w.rt.set_model(&"m".repeat(128));
        let line = w.turn_line();
        assert!(line.len() <= TURN_LINE_CAP, "len={}", line.len());
        assert!(line.starts_with("mmm"));
        assert!(line.contains("turns="));
    }

    #[test]
    fn double_finish_saturates() {
        let mut w = TurnWire::new();
        w.begin();
        w.finish();
        w.finish();
        assert_eq!(w.rt.turns(), 2);
        assert_eq!(w.turn_line(), " turns=2 false");
    }
}
