//! Pinned normal-TUI logo shapes shared by text layout and native painting.
//!
//! Upstream evidence (OpenCode `450e7b92412c780a7b5a75226bb8d49aaca24e22`):
//! - `packages/tui/src/logo.ts:1-9` — the `logo.left`/`logo.right` and
//!   `go.left`/`go.right` string arrays, including the `_ ^ ~ ,` markers.
//! - `packages/tui/src/component/logo.tsx:54-76` — breakpoints: hidden below
//!   12 rows, `go.right.slice(1)` below 22 columns (3 rows, base/bold), then
//!   `logo.left.slice(1)` (muted) followed by `logo.right` (base/bold) below
//!   44 columns (7 rows), else the 4-row side-by-side lockup with `gap={1}`.
//! - `packages/tui/src/component/logo.tsx:12-49` — `renderLine` marker
//!   substitutions: `_` → space on the tinted shadow background, `^` → `▀` on
//!   the shadow background, `~` → `▀` in the shadow foreground, `,` → `▄` in
//!   the shadow foreground; every other character keeps the line foreground.
//!   The left half renders in `theme.text.muted` and the right half in
//!   `theme.text.base` (bold).

/// Foreground role of a logo cell. `Shadow` is `tint(background.base, fg,
/// 0.25)` in `component/logo.tsx:13`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ink {
    Base,
    Muted,
    Shadow,
}

/// One painted logo cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Cell {
    /// Glyph after marker substitution.
    pub glyph: char,
    /// Foreground role.
    pub ink: Ink,
    /// Whether the cell paints the tinted shadow background (`bg={shadow}`).
    pub shadow_background: bool,
}

/// Apply upstream `renderLine` substitutions to one raw logo row.
#[must_use]
pub(crate) fn render_line(raw: &str, ink: Ink) -> Vec<Cell> {
    raw.chars()
        .map(|marker| match marker {
            '_' => Cell { glyph: ' ', ink, shadow_background: true },
            '^' => Cell { glyph: '▀', ink, shadow_background: true },
            '~' => Cell { glyph: '▀', ink: Ink::Shadow, shadow_background: false },
            ',' => Cell { glyph: '▄', ink: Ink::Shadow, shadow_background: false },
            other => Cell { glyph: other, ink, shadow_background: false },
        })
        .collect()
}

/// Ink for a lockup half: the left half is muted, the right half base+bold.
#[must_use]
pub(crate) const fn ink(bold: bool) -> Ink {
    if bold {
        Ink::Base
    } else {
        Ink::Muted
    }
}

pub(crate) struct Segment {
    pub x: u16,
    pub y: u16,
    pub raw: &'static str,
    pub bold: bool,
}

const LEFT: [&str; 4] = ["                   ", "█▀▀█ █▀▀█ █▀▀█ █▀▀▄", "█__█ █__█ █^^^ █__█", "▀▀▀▀ █▀▀▀ ▀▀▀▀ ▀~~▀"];
const RIGHT: [&str; 4] = ["             ▄     ", "█▀▀▀ █▀▀█ █▀▀█ █▀▀█", "█___ █__█ █__█ █^^^", "▀▀▀▀ ▀▀▀▀ ▀▀▀▀ ▀▀▀▀"];

pub(crate) fn dimensions(cols: u16, rows: u16) -> (u16, u16) {
    if rows < 12 { (0, 0) }
    else if cols < 22 { (4, 3) }
    else if cols < 44 { (19, 7) }
    else { (39, 4) }
}

pub(crate) fn segments(cols: u16, rows: u16) -> Vec<Segment> {
    let (_, height) = dimensions(cols, rows);
    match height {
        3 => ["█▀▀█", "█__█", "▀▀▀▀"].into_iter().enumerate()
            .map(|(y, raw)| Segment { x: 0, y: y as u16, raw, bold: true }).collect(),
        7 => LEFT[1..].iter().map(|raw| (*raw, false)).chain(RIGHT.iter().map(|raw| (*raw, true)))
            .enumerate().map(|(y, (raw, bold))| Segment { x: 0, y: y as u16, raw, bold }).collect(),
        4 => (0..4).flat_map(|y| [Segment { x: 0, y, raw: LEFT[y as usize], bold: false },
            Segment { x: 20, y, raw: RIGHT[y as usize], bold: true }]).collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_breakpoints_match_the_pinned_component() {
        // Hidden below 12 rows.
        assert_eq!(dimensions(80, 11), (0, 0));
        assert!(segments(80, 11).is_empty());
        assert_eq!(dimensions(80, 12), (39, 4));
        // < 22 columns: `go.right.slice(1)`, 4x3, base/bold.
        assert_eq!(dimensions(21, 24), (4, 3));
        let compact = segments(21, 24);
        assert_eq!(compact.len(), 3);
        assert_eq!(compact.iter().map(|s| s.raw).collect::<Vec<_>>(), ["█▀▀█", "█__█", "▀▀▀▀"]);
        assert!(compact.iter().all(|s| s.bold && s.x == 0));
        assert!(compact.iter().all(|s| s.raw.chars().count() == 4));
        // 22..43: muted left (slice(1)) stacked over the bold right, 19x7.
        assert_eq!(dimensions(22, 24), (19, 7));
        assert_eq!(dimensions(43, 24), (19, 7));
        let stacked = segments(43, 24);
        assert_eq!(stacked.len(), 7);
        assert!(stacked.iter().all(|s| s.x == 0 && s.raw.chars().count() == 19));
        assert!(stacked[..3].iter().all(|s| !s.bold), "first three rows are muted");
        assert!(stacked[3..].iter().all(|s| s.bold), "last four rows are base/bold");
        // >= 44: side-by-side lockup, one-cell gap, x = 0/20 (left is 19 wide).
        assert_eq!(dimensions(44, 24), (39, 4));
        let lockup = segments(80, 24);
        assert_eq!(lockup.len(), 8);
        for y in 0..4u16 {
            let left = lockup.iter().find(|s| s.y == y && !s.bold).expect("left segment");
            let right = lockup.iter().find(|s| s.y == y && s.bold).expect("right segment");
            assert_eq!(left.x, 0);
            assert_eq!(left.raw.chars().count(), 19);
            assert_eq!(right.x, 20, "left width 19 + component gap 1");
            assert_eq!(right.raw.chars().count(), 19);
            assert_eq!(left.x + 19 + 1, right.x);
        }
        // Total lockup width fits the 39-column contract.
        assert!(lockup.iter().all(|s| u32::from(s.x) + s.raw.chars().count() as u32 <= 39));
    }

    #[test]
    fn logo_cells_apply_upstream_glyph_and_shading_rules() {
        // Compact marker row: base glow, shadow background and shadow fg.
        let cells = render_line("█_^~,", Ink::Base);
        assert_eq!(cells.len(), 5);
        assert_eq!(cells[0], Cell { glyph: '█', ink: Ink::Base, shadow_background: false });
        assert_eq!(cells[1], Cell { glyph: ' ', ink: Ink::Base, shadow_background: true });
        assert_eq!(cells[2], Cell { glyph: '▀', ink: Ink::Base, shadow_background: true });
        assert_eq!(cells[3], Cell { glyph: '▀', ink: Ink::Shadow, shadow_background: false });
        assert_eq!(cells[4], Cell { glyph: '▄', ink: Ink::Shadow, shadow_background: false });

        // Muted left half keeps the muted ink for its surface markers.
        let muted = render_line("█_█", Ink::Muted);
        assert!(muted.iter().all(|cell| cell.ink == Ink::Muted));
        assert_eq!(muted[1].glyph, ' ');
        assert!(muted[1].shadow_background);

        // The real lockup rows paint without leaking marker characters, keep
        // their pinned widths, and carry the right shading roles.
        for (raw, want_ink) in LEFT.iter().map(|raw| (*raw, Ink::Muted))
            .chain(RIGHT.iter().map(|raw| (*raw, Ink::Base)))
        {
            let painted = render_line(raw, want_ink);
            assert_eq!(painted.len(), raw.chars().count());
            assert_eq!(painted.len(), 19);
            assert!(painted.iter().all(|cell| !matches!(cell.glyph, '_' | '^' | '~' | ',')));
            assert!(painted.iter().all(|cell| cell.ink == want_ink || cell.ink == Ink::Shadow));
        }

        // `ink(bold)` maps the left half to muted and the right half to base.
        assert_eq!(ink(false), Ink::Muted);
        assert_eq!(ink(true), Ink::Base);
    }
}
