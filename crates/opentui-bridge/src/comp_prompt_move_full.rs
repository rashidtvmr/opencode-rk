#![forbid(unsafe_code)]
//! Prompt cursor move (TS `packages/tui/src/component/prompt/move.tsx:18` `usePromptMove`).
//! Minimal cursor: `pos` in `0..=len`, clamped steps.

/// Cursor over a prompt buffer of `len` chars; `pos` is the gap index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromptMove {
    pos: usize,
    len: usize,
}

impl PromptMove {
    /// New cursor at start of a `len`-char buffer.
    #[must_use]
    pub fn new(len: usize) -> Self {
        Self { pos: 0, len }
    }

    /// Move by `delta`, clamped to `0..=len`; empty buffer stays at 0.
    pub fn step(&mut self, delta: isize) {
        let end = self.len as isize;
        self.pos = (self.pos as isize + delta).clamp(0, end) as usize;
    }

    /// Current gap index.
    #[must_use]
    pub fn pos(&self) -> usize {
        self.pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_zero() {
        assert_eq!(PromptMove::new(5).pos(), 0);
    }

    #[test]
    fn moves_within_bounds() {
        let mut m = PromptMove::new(5);
        m.step(2);
        assert_eq!(m.pos(), 2);
        m.step(-1);
        assert_eq!(m.pos(), 1);
    }

    #[test]
    fn clamps_both_ends() {
        let mut m = PromptMove::new(3);
        m.step(99);
        assert_eq!(m.pos(), 3);
        m.step(-99);
        assert_eq!(m.pos(), 0);
    }

    #[test]
    fn empty_stays_zero() {
        let mut m = PromptMove::new(0);
        m.step(1);
        m.step(-1);
        assert_eq!(m.pos(), 0);
    }
}
