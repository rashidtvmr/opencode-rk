#![forbid(unsafe_code)]
//! Palette exec bounds (palette cannot execute yet).
//! Documents [`crate::session_commands::COMMANDS`] 31 flat ids.
//! `ponytail:` CMD_COUNT + hint cap only; no title/value/category/
//! enabled/slash/run vs `index.tsx:458`; 12 static hint strings.

/// Flat command count (see `session_commands.rs:15`).
pub const CMD_COUNT: usize = 31;
/// Max static hint strings shown.
pub const PALETTE_HINT_MAX: usize = 12;

/// In-bounds command index (`0..CMD_COUNT`)?
#[must_use]
pub fn cmd_at(i: usize) -> bool {
    i < CMD_COUNT
}

/// Cap visible hint count at [`PALETTE_HINT_MAX`].
#[must_use]
pub fn palette_trunc(n: usize) -> usize {
    n.min(PALETTE_HINT_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_match_sources() {
        assert_eq!(CMD_COUNT, 31);
        assert_eq!(PALETTE_HINT_MAX, 12);
    }

    #[test]
    fn bounds_edges() {
        assert!(cmd_at(0));
        assert!(cmd_at(CMD_COUNT - 1));
        assert!(!cmd_at(CMD_COUNT));
        assert!(!cmd_at(usize::MAX));
    }

    #[test]
    fn trunc_passthrough() {
        assert_eq!(palette_trunc(0), 0);
        assert_eq!(palette_trunc(5), 5);
        assert_eq!(palette_trunc(PALETTE_HINT_MAX), 12);
    }

    #[test]
    fn trunc_caps() {
        assert_eq!(palette_trunc(13), 12);
        assert_eq!(palette_trunc(1000), 12);
        assert_eq!(palette_trunc(usize::MAX), 12);
    }
}
