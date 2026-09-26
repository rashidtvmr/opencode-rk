#![forbid(unsafe_code)]
//! Atomic fenced-paste cap: `START payload END` decodes as one `Paste`.
//!
//! TS truth: `prompt/index.tsx` `:1393,1402,1434` bracketed-paste.
//! Divergence 1: fenced paste fed per-char through per-char `type:` path
//! explodes into one event per byte instead of one `Paste`.
//! Divergence 2: bare `PASTE_START`/`PASTE_END` markers are misrouted to
//! `Focus` at `input_events.rs:82-87`; assemble fenced first via this cap.

/// Max pasted bytes collected between PASTE markers.
pub const MAX_PASTE: usize = 4096;

/// Byte length of accepted fenced-paste payload. Bounded, saturating.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PasteBuf {
    pub len: usize,
}

impl PasteBuf {
    /// Empty payload.
    #[must_use]
    pub const fn new() -> Self {
        Self { len: 0 }
    }

    /// Reserve `n` bytes; `false` (no growth) past [`MAX_PASTE`].
    pub fn push_ok(&mut self, n: usize) -> bool {
        let next = self.len.saturating_add(n);
        if next > MAX_PASTE {
            return false;
        }
        self.len = next;
        true
    }
}

/// Head byte of a bare paste marker (`ESC` opens `ESC[200~`/`ESC[201~`).
#[must_use]
pub const fn is_bare_marker(b: u8) -> bool {
    b == 0x1b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_allows_up_to_max() {
        let mut p = PasteBuf::new();
        assert!(p.push_ok(MAX_PASTE));
        assert_eq!(p.len, MAX_PASTE);
    }

    #[test]
    fn cap_rejects_overflow() {
        let mut p = PasteBuf::new();
        assert!(p.push_ok(MAX_PASTE));
        assert!(!p.push_ok(1));
        assert_eq!(p.len, MAX_PASTE);
    }

    #[test]
    fn saturating_no_wrap() {
        let mut p = PasteBuf::new();
        assert!(!p.push_ok(usize::MAX));
        assert_eq!(p.len, 0);
    }

    #[test]
    fn bare_marker_esc_only() {
        assert!(is_bare_marker(0x1b));
        assert!(!is_bare_marker(b'['));
        assert!(!is_bare_marker(b'a'));
    }
}
