#![forbid(unsafe_code)]
//! u16 saturating shell splits + overlap probe (FIX-26).
//! `ponytail:` thin u16 mirror of layout.rs solver; full Constraint engine stays there.

/// Min full-shell viewport (mirrors native_layout MIN_FULL_WIDTH/HEIGHT).
pub const MIN_W: u16 = 80;
pub const MIN_H: u16 = 24;

/// Integer cell rect, origin top-left.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl Rect {
    #[must_use]
    pub const fn new(x: u16, y: u16, w: u16, h: u16) -> Self {
        Self { x, y, w, h }
    }
}

/// Split height `h` into `(top, bottom)`; clamps `top` to `h`.
#[must_use]
pub const fn split_row(h: u16, top: u16) -> (u16, u16) {
    let t = if top < h { top } else { h };
    (t, h.saturating_sub(t))
}

/// Split width `w` into `(left, right)`; clamps `left` to `w`.
#[must_use]
pub const fn split_col(w: u16, left: u16) -> (u16, u16) {
    let l = if left < w { left } else { w };
    (l, w.saturating_sub(l))
}

/// True when interiors intersect; touching edges and zero areas never overlap.
#[must_use]
pub const fn has_overlap(a: Rect, b: Rect) -> bool {
    if a.w == 0 || a.h == 0 || b.w == 0 || b.h == 0 {
        return false;
    }
    a.x < b.x.saturating_add(b.w)
        && b.x < a.x.saturating_add(a.w)
        && a.y < b.y.saturating_add(b.h)
        && b.y < a.y.saturating_add(a.h)
}

/// Canonical-source declaration for shell layout ownership.
#[must_use]
pub const fn canonical_note() -> &'static str {
    "Canonical: opentui-bridge/src/layout.rs ShellRegions::compute. Dupe: world.rs ShellRegions::new (container only, no solver) and cli/src/native_layout.rs ShellLayout (solver dupe); has_overlap proofs never enforced, call it after compute."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_conserve_total() {
        assert_eq!(split_row(40, 5), (5, 35));
        assert_eq!(split_col(120, 30), (30, 90));
    }

    #[test]
    fn splits_clamp_saturating() {
        assert_eq!(split_row(3, 99), (3, 0));
        assert_eq!(split_col(0, 7), (0, 0));
        assert_eq!(split_row(u16::MAX, u16::MAX), (u16::MAX, 0));
    }

    #[test]
    fn overlap_detects_interior_only() {
        let a = Rect::new(0, 0, 50, 20);
        let b = Rect::new(0, 10, 50, 10);
        assert!(has_overlap(a, b));
        assert!(!has_overlap(a, Rect::new(50, 0, 10, 20)));
        assert!(!has_overlap(a, Rect::new(0, 0, 0, 0)));
    }

    #[test]
    fn min_consts_match_full_shell() {
        assert_eq!((MIN_W, MIN_H), (80, 24));
        assert_eq!(split_row(MIN_H, 5), (5, 19));
    }

    #[test]
    fn note_names_canonical_and_dupes() {
        let n = canonical_note();
        assert!(n.contains("layout.rs"));
        assert!(n.contains("world.rs"));
        assert!(n.contains("native_layout.rs"));
    }
}
