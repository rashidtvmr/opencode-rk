#![forbid(unsafe_code)]
//! Resize memo invalidation gap (FIX-46): width-cached memos (`clip_line`
//! clip/pad, `page_adapter::page_lines` trunc/pad only) go stale on resize.
//! Evidence: `solid_host.rs:116-121` (`is_narrow`, `width < 80`).

/// Narrow-layout threshold (mirrors `solid_host` `width < 80`).
pub const NARROW_LT: u16 = 80;

/// Narrow-layout predicate for a raw column count.
#[must_use]
pub const fn is_narrow(w: u16) -> bool {
    w < NARROW_LT
}

/// Monotonic memo version; bumped on resize-invalidation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemoVer {
    pub ver: u64,
}

impl MemoVer {
    /// Saturating bump; never wraps.
    pub fn bump(&mut self) {
        self.ver = self.ver.saturating_add(1);
    }
}

/// Width-keyed memo cell: cached width + version + dirty flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemoCell {
    pub width: u16,
    pub memo: MemoVer,
    pub dirty: bool,
}

impl MemoCell {
    /// On new width: record it, bump version, set dirty. Same width: no-op.
    pub fn invalidate(&mut self, width: u16) {
        if self.width != width {
            // ponytail: one cell; upgrade to per-memo map when callers need it.
            self.width = width;
            self.memo.bump();
            self.dirty = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_matches_host() {
        assert!(is_narrow(79));
        assert!(!is_narrow(80));
    }

    #[test]
    fn bump_saturates() {
        let mut m = MemoVer { ver: u64::MAX };
        m.bump();
        assert_eq!(m.ver, u64::MAX);
    }

    #[test]
    fn invalidate_sets_dirty() {
        let mut c = MemoCell::default();
        c.invalidate(80);
        assert!((c.dirty, c.width, c.memo.ver) == (true, 80, 1));
    }

    #[test]
    fn invalidate_noop_same_width() {
        let mut c = MemoCell {
            width: 80,
            memo: MemoVer { ver: 3 },
            dirty: false,
        };
        c.invalidate(80);
        assert!(!c.dirty && c.memo.ver == 3);
    }
}
