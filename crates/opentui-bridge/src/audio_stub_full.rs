#![forbid(unsafe_code)]
//! Muted playback stub (mirrors `packages/tui/src/audio.ts`).
//!
//! TS truth (`audio.ts:38-43`): `play` returns null when the audio context
//! is missing; the stub models that as `muted` (play returns false).
//!
//! `ponytail:` flag + saturating counter only; upgrade path is a real host
//! audio handle, add only when a caller needs sound output.

/// Playback stub; muted swallows plays, unmuted counts them.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AudioStub {
    pub muted: bool,
    pub plays: u32,
}

impl AudioStub {
    /// Unmuted stub with zero plays.
    #[must_use]
    pub fn new() -> Self {
        Self {
            muted: false,
            plays: 0,
        }
    }

    /// False when muted, else bumps plays and returns true.
    pub fn play(&mut self) -> bool {
        if self.muted {
            return false;
        }
        self.plays = self.plays.saturating_add(1);
        true
    }

    /// Mute (`true`) or unmute (`false`) playback.
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Successful play count.
    #[must_use]
    pub fn plays(&self) -> u32 {
        self.plays
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unmuted_play_counts() {
        let mut s = AudioStub::new();
        assert!(s.play());
        assert!(s.play());
        assert_eq!(s.plays(), 2);
    }

    #[test]
    fn muted_play_is_noop() {
        let mut s = AudioStub {
            muted: true,
            plays: 0,
        };
        assert!(!s.play());
        assert_eq!(s.plays(), 0);
    }

    #[test]
    fn set_muted_toggles() {
        let mut s = AudioStub::new();
        s.set_muted(true);
        assert!(!s.play());
        s.set_muted(false);
        assert!(s.play());
        assert_eq!(s.plays(), 1);
    }
}
