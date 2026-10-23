#![forbid(unsafe_code)]
//! Main-column shell geometry and frame composition.
//!
//! Pure and synchronous: geometry ([`Layout`]) plus a frame composer. No IO,
//! no clock, no backend/state access, no ANSI output. Every returned row is
//! plain text whose display width is exactly the viewport width, so painting
//! the full frame clears the previous region cell-exactly.
//!
//! Contract: upstream `packages/tui/src/routes/session/index.tsx:255-278`
//! (SPEC `upstream-tui-layout-parity-v1`, pin
//! `95daf90670b7c039c436c85537da5fbfe2205b41`) has no permanent left
//! all-sessions pane and no permanent right status/MCP pane. The main area is
//! one growing column. An inline sidebar is a FIXED 42-column box, present
//! only when `cols > 120` (strict) and the height can host it; the left width
//! is always zero and no separator cell is drawn:
//! `left_width + center_width + right_width == cols`.
//!
//! The upstream view padding (`contentWidth = cols - 42 - 4`) is applied by
//! the view, NOT folded into the layout: [`Layout::center_width`] is the outer
//! `cols - right_width`. Zero or tiny viewports are safe: no out-of-range
//! cells, no UTF-8 slicing, bounded output.
//!
//! Rendering is a bounded streaming scan: each line is inspected for at most
//! `width * MAX_CLUSTER_BYTES + SCAN_OVERHEAD_BYTES` bytes (so an
//! unterminated CSI/OSC/DCS prefix stops at a bounded prefix instead of
//! hunting for a terminator), escape sequences and control scalars are
//! stripped while scanning, and only bounded clusters are retained. Content
//! is grouped into bounded clusters (base + combining marks / variation
//! selectors / skin modifiers / ZWJ-joined bases) and truncation happens on
//! cluster boundaries only: no dangling ZWJ, orphan variation selector, half
//! flag or combining-only atom is ever emitted.

use std::cmp::{max, min};

/// Fixed inline sidebar width when the wide layout is active (upstream
/// `session/index.tsx`: an inline box of exactly 42 columns).
const SIDEBAR_WIDTH: u16 = 42;
/// Strict width threshold: the sidebar appears only when `cols > 120`
/// (upstream `wide = terminal_width > 120`).
const WIDE_MIN_COLS: u16 = 120;
/// Hard cap on composed frame cells (`cols * rows`), so a hostile or absurd
/// viewport cannot allocate unbounded memory.
const MAX_FRAME_CELLS: usize = 1 << 20;
/// Max bytes retained for one rendered cluster (base plus bounded
/// continuations). Bounds retained bytes per cell and the per-line scan
/// budget (`width * MAX_CLUSTER_BYTES + SCAN_OVERHEAD_BYTES`).
const MAX_CLUSTER_BYTES: usize = 48;
/// Max zero-width continuations (combining marks, variation selectors,
/// skin modifiers, ZWJ) kept per cluster; further marks are dropped.
const MAX_CONTINUATIONS: usize = 8;
/// Max base scalars per cluster, bounding ZWJ join chains.
const MAX_CLUSTER_BASES: usize = 4;
/// Fixed per-line scan overhead on top of the per-cell budget, so short
/// panes can still consume ordinary escape prefixes before the bounded
/// prefix limit stops the scan.
const SCAN_OVERHEAD_BYTES: usize = 256;
/// ZWJ (U+200D): joins the next base into one cluster.
const ZWJ: char = '\u{200D}';
/// Below this viewport height the shell cannot reserve a composer row, so the
/// sidebar is suppressed (height-safe guard).
const MIN_FULL_HEIGHT: u16 = 2;
/// A completed regional-indicator (flag) pair renders as one 2-cell glyph;
/// never the naive scalar sum of its two RI halves.
const FLAG_PAIR_CELLS: usize = 2;

/// Viewport geometry for the native shell frame.
///
/// Widths are outer column counts. `left_width` is always zero (upstream has
/// no permanent left all-sessions pane). `right_width` is the fixed 42-column
/// sidebar when active, else zero. `center_width == cols - right_width`; no
/// separator cells exist between columns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Layout {
    /// Viewport width in cells.
    pub cols: u16,
    /// Viewport height in rows.
    pub rows: u16,
    /// Left pane width in cells: always zero.
    pub left_width: u16,
    /// Main-column outer width in cells (`cols - right_width`).
    pub center_width: u16,
    /// Right sidebar outer width in cells: `SIDEBAR_WIDTH` or zero.
    pub right_width: u16,
    /// True for a zero-area viewport (nothing to paint).
    pub compact: bool,
}

impl Layout {
    /// Geometry for a `cols x rows` viewport.
    ///
    /// `right_width = SIDEBAR_WIDTH` only when `cols > WIDE_MIN_COLS` and
    /// `rows >= MIN_FULL_HEIGHT`; otherwise zero. `center_width` takes the
    /// remaining columns and `left_width` is always zero, so
    /// `left_width + center_width + right_width == cols`. `compact` flags a
    /// zero-area viewport; any `cols`/`rows` (including zero) is safe.
    #[must_use]
    pub fn new(cols: u16, rows: u16) -> Layout {
        let right_width = if cols > WIDE_MIN_COLS && rows >= MIN_FULL_HEIGHT {
            SIDEBAR_WIDTH
        } else {
            0
        };
        Layout {
            cols,
            rows,
            left_width: 0,
            center_width: cols.saturating_sub(right_width),
            right_width,
            compact: cols == 0 || rows == 0,
        }
    }
}

/// Compose one frame from caller-owned main-column content.
///
/// `center` is the main column (chat/page/dialog body) and `right` is the
/// optional sidebar; `left` is accepted for API compatibility and ignored —
/// upstream has no permanent left all-sessions pane at any width, so its
/// content must never be painted. Every row is exactly `layout.cols` cells
/// wide (missing rows are blank; extra content is clipped on a cell boundary:
/// wide glyphs are never split, combining marks stay with their base, control
/// bytes and ANSI sequences never reach the frame). At most
/// `min(rows, cell_budget / cols)` rows are emitted; zero-area viewports emit
/// nothing.
pub fn compose_frame(
    layout: &Layout,
    _left: &[String],
    center: &[String],
    right: &[String],
) -> Vec<String> {
    let cols = usize::from(layout.cols);
    let rows = usize::from(layout.rows);
    if cols == 0 || rows == 0 {
        return Vec::new();
    }
    let row_budget = max(1, MAX_FRAME_CELLS / cols);
    let used = min(rows, row_budget);
    let mut frame = Vec::with_capacity(used);
    for row in 0..used {
        // Main column first, then the optional fixed sidebar, concatenated
        // with no separator cell. The sidebar is the exception, not a
        // permanent status pane.
        let mut line = fit(line_at(center, row), layout.center_width);
        if layout.right_width > 0 {
            line.push_str(&fit(line_at(right, row), layout.right_width));
        }
        // Trust-boundary normalization: even a hand-built `Layout` with
        // inconsistent widths yields an in-range, exactly-wide row.
        frame.push(fit(&line, layout.cols));
    }
    frame
}

/// Row `row` of a pane slice, or an empty line when the caller passed fewer
/// lines than the viewport has rows.
fn line_at(lines: &[String], row: usize) -> &str {
    lines.get(row).map_or("", String::as_str)
}

/// Bounded per-line scan budget in bytes: a per-cell cluster byte budget
/// plus fixed escape overhead. An unterminated CSI/OSC/DCS prefix therefore
/// stops at a bounded prefix instead of scanning the whole input for a
/// terminator that may never come.
fn scan_budget(width: usize) -> usize {
    width
        .saturating_mul(MAX_CLUSTER_BYTES)
        .saturating_add(SCAN_OVERHEAD_BYTES)
}

/// Stream `line` into exactly `width` display cells without copying the
/// input: escape sequences and control scalars are stripped while scanning
/// (finite prefix scan, raw escape bytes are never emitted), content is
/// grouped into bounded clusters, and truncation happens only on cluster
/// boundaries — a cluster that would exceed the remaining width is dropped
/// whole and the tail is padded visibly. Retained bytes stay bounded by
/// `width * MAX_CLUSTER_BYTES + width` (clusters plus padding); the scan
/// itself stops at [`scan_budget`].
pub(crate) fn fit(line: &str, width: u16) -> String {
    let mut out = String::with_capacity(usize::from(width));
    let used = visit_unpadded_cells(line, usize::from(width), |glyph, _| out.push_str(glyph));
    out.extend(std::iter::repeat_n(' ', usize::from(width) - used));
    out
}

/// Visit bounded sanitized clusters without adding padding. Shares fit's exact
/// segmentation so measurement and wrapping cannot split a displayed cluster.
pub(crate) fn visit_unpadded_cells(line: &str, budget: usize, mut visit: impl FnMut(&str, usize)) -> usize {
    if budget == 0 {
        return 0;
    }
    let scan_limit = scan_budget(budget);
    let mut used = 0usize;
    let mut cluster = Cluster::default();
    let mut state = Escape::Normal;
    for (index, raw) in line.char_indices() {
        if index >= scan_limit {
            break;
        }
        let next: Option<char> = match state {
            Escape::Normal => {
                if raw == '\x1b' {
                    state = Escape::Esc;
                    None
                } else if raw.is_control() {
                    // C0/C1/DEL become an ordinary space cell.
                    Some(' ')
                } else {
                    Some(raw)
                }
            }
            // ESC + one byte: consume the two-byte sequence, then resume.
            Escape::Esc => {
                state = match raw {
                    '[' => Escape::Csi,
                    ']' | 'P' | 'X' | '^' | '_' => Escape::String,
                    _ => Escape::Normal,
                };
                None
            }
            // CSI: swallow parameters/intermediates through the final byte.
            Escape::Csi => {
                if ('\x40'..='\x7e').contains(&raw) {
                    state = Escape::Normal;
                }
                None
            }
            // OSC/DCS/SOS/PM/APC string: swallow until BEL or ST (ESC \).
            Escape::String => match raw {
                '\x07' => {
                    state = Escape::Normal;
                    None
                }
                '\x1b' => {
                    state = Escape::Esc;
                    None
                }
                _ => None,
            },
        };
        let Some(c) = next else { continue };
        if is_continuation(raw) {
            // A leading continuation is dropped: never a combining-only
            // atom, an orphan VS or a dangling ZWJ without a base.
            if !cluster.is_empty() {
                cluster.push_cont(raw);
            }
            continue;
        }
        let w = usize::from(char_width(c));
        if cluster.is_empty() {
            if used >= budget {
                break;
            }
            cluster.push_base(c, w);
            if is_regional(raw) {
                cluster.regional = 1;
            }
            continue;
        }
        if cluster.last_was_zwj && cluster.can_join(c) {
            // ZWJ + following base stays one cluster (bounded join chain).
            cluster.join(c, w);
            continue;
        }
        if is_regional(raw) && cluster.regional == 1 && cluster.bases == 1 && cluster.conts == 0 {
            // Second regional indicator completes the flag pair.
            cluster.join(c, w);
            cluster.regional = 2;
            continue;
        }
        // Previous cluster complete: emit it if it fits, otherwise truncate
        // here on the cluster boundary and pad the tail visibly.
        if let Some((text, cell_width)) = cluster.finish() {
            if used + cell_width > budget {
                break;
            }
            visit(&text, cell_width);
            used += cell_width;
            if used >= budget {
                break;
            }
        }
        cluster.push_base(c, w);
        if is_regional(raw) {
            cluster.regional = 1;
        }
    }
    // End of input or scan budget: finish the pending cluster (dropping a
    // dangling trailing ZWJ, a lone regional indicator or an empty cluster)
    // when it still fits.
    if let Some((text, cell_width)) = cluster.finish() {
        if used + cell_width <= budget {
            visit(&text, cell_width);
            used += cell_width;
        }
    }
    used
}

/// Visit sanitized display clusters, preserving the same bounded segmentation
/// used by `fit`. Native styling can change at cell coordinates without
/// splitting combining sequences, flags or joined emoji.
pub(crate) fn visit_cells(line: &str, width: u16, mut visit: impl FnMut(u16, &str, u16)) {
    let text = fit(line, width);
    let mut cluster = Cluster::default();
    let mut x = 0u16;
    for c in text.chars() {
        if is_continuation(c) {
            if !cluster.is_empty() {
                cluster.push_cont(c);
            }
            continue;
        }
        if !cluster.is_empty()
            && ((cluster.last_was_zwj && cluster.can_join(c))
                || (is_regional(c)
                    && cluster.regional == 1
                    && cluster.bases == 1
                    && cluster.conts == 0))
        {
            let flag = is_regional(c) && cluster.regional == 1;
            cluster.join(c, usize::from(char_width(c)));
            if flag {
                cluster.regional = 2;
            }
            continue;
        }
        if let Some((glyph, cells)) = cluster.finish() {
            visit(x, &glyph, cells as u16);
            x = x.saturating_add(cells as u16);
        }
        cluster.push_base(c, usize::from(char_width(c)));
        if is_regional(c) {
            cluster.regional = 1;
        }
    }
    if let Some((glyph, cells)) = cluster.finish() {
        visit(x, &glyph, cells as u16);
    }
}

/// Extract a display-cell interval; glyphs crossing either boundary become
/// spaces rather than partial Unicode clusters.
pub(crate) fn slice_cells(line: &str, start: u16, width: u16) -> String {
    let end = start.saturating_add(width);
    let mut result = String::new();
    let mut used = 0u16;
    visit_cells(line, end, |x, glyph, cells| {
        if x >= start && x.saturating_add(cells) <= end {
            let offset = x - start;
            while used < offset {
                result.push(' ');
                used += 1;
            }
            result.push_str(glyph);
            used += cells;
        }
    });
    while used < width {
        result.push(' ');
        used += 1;
    }
    result
}

/// True for zero-width cluster continuations: combining marks, variation
/// selectors, format characters (including ZWJ) and skin-tone modifiers.
/// These attach to a preceding base only; a leading one is dropped.
fn is_continuation(c: char) -> bool {
    let cp = c as u32;
    in_ranges(cp, ZERO_WIDTH_RANGES) || (0x1F3FB..=0x1F3FF).contains(&cp)
}

/// True for regional indicator scalars (flag halves; pairs form one cluster).
fn is_regional(c: char) -> bool {
    (0x1F1E6..=0x1F1FF).contains(&(c as u32))
}

/// Escape-sequence scanner state for [`fit`].
#[derive(Clone, Copy, Eq, PartialEq)]
enum Escape {
    Normal,
    Esc,
    Csi,
    String,
}

/// One bounded rendered cluster: a base scalar plus at most
/// [`MAX_CONTINUATIONS`] zero-width continuations (combining marks,
/// variation selectors, skin modifiers, ZWJ) and at most
/// [`MAX_CLUSTER_BASES`] bases for ZWJ join chains.
///
/// `ponytail:` cluster recognition is a heuristic subset of full grapheme
/// segmentation (base + marks + modifiers + ZWJ join + flag pair), bounded
/// and std-only for pane clipping; it does not implement every UAX #29 rule.
/// Upgrade path: the `unicode-segmentation` crate when dependency additions
/// are permitted.
#[derive(Default)]
struct Cluster {
    /// Rendered scalars of this cluster (bounded by `MAX_CLUSTER_BYTES`).
    text: String,
    /// Naive sum of base widths.
    sum: usize,
    /// Widest base width (joined-ZWJ heuristic).
    max: usize,
    /// Base scalars stored.
    bases: usize,
    /// Zero-width continuations stored.
    conts: usize,
    /// A ZWJ scalar is stored (cluster is join-shaped).
    zwj: bool,
    /// The current base can be joined (previous stored scalar was ZWJ).
    last_was_zwj: bool,
    /// Regional indicators stored: 0 none, 1 lone, 2 complete flag.
    regional: usize,
}

impl Cluster {
    fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Start an empty cluster with `c` as its base.
    fn push_base(&mut self, c: char, w: usize) {
        self.text.push(c);
        self.sum = w;
        self.max = w;
        self.bases = 1;
        self.conts = 0;
        self.zwj = false;
        self.last_was_zwj = false;
        self.regional = 0;
    }

    /// Append a zero-width continuation while within the byte and count
    /// caps; beyond the caps the scalar is dropped, never the cluster.
    fn push_cont(&mut self, c: char) {
        if self.conts >= MAX_CONTINUATIONS || self.text.len() + c.len_utf8() > MAX_CLUSTER_BYTES {
            return;
        }
        self.text.push(c);
        self.conts += 1;
        if c == ZWJ {
            self.zwj = true;
            self.last_was_zwj = true;
        }
    }

    /// Join a following base (after ZWJ) or the second regional indicator.
    fn join(&mut self, c: char, w: usize) {
        self.text.push(c);
        self.sum += w;
        if w > self.max {
            self.max = w;
        }
        self.bases += 1;
        self.last_was_zwj = false;
    }

    /// Whether `c` joins the current cluster within the cluster caps.
    fn can_join(&self, c: char) -> bool {
        self.bases < MAX_CLUSTER_BASES && self.text.len() + c.len_utf8() <= MAX_CLUSTER_BYTES
    }

    /// Complete the cluster: a trailing ZWJ is removed (never a dangling
    /// join), a lone regional indicator is dropped (never a half flag), an
    /// empty cluster yields nothing (never a combining-only atom). Returns
    /// the text and its cell width: the widest base for a ZWJ-joined cluster
    /// of two or more bases (joined emoji occupy one cell group), otherwise
    /// the naive sum of base widths.
    fn finish(&mut self) -> Option<(String, usize)> {
        while self.text.ends_with(ZWJ) {
            self.text.pop();
        }
        if self.text.is_empty() || self.regional == 1 {
            self.reset();
            return None;
        }
        let width = if self.regional == 2 {
            // A completed flag pair is one 2-cell glyph; never the naive
            // scalar sum of its halves (4 when the width table classifies
            // regional indicators as wide).
            FLAG_PAIR_CELLS
        } else if self.zwj && self.bases >= 2 {
            self.max
        } else {
            self.sum
        };
        let text = std::mem::take(&mut self.text);
        self.reset();
        Some((text, width))
    }

    fn reset(&mut self) {
        self.text.clear();
        self.sum = 0;
        self.max = 0;
        self.bases = 0;
        self.conts = 0;
        self.zwj = false;
        self.last_was_zwj = false;
        self.regional = 0;
    }
}

/// Display cells occupied by `c`: 0 for combining/zero-width marks, 2 for
/// wide (CJK, fullwidth, emoji) scalars, 1 otherwise.
///
/// `ponytail:` bounded std-only approximation of East Asian Width + grapheme
/// cluster behavior (no Unicode tables in std, no new Cargo deps allowed for
/// this part). Ceiling: a handful of edge emoji/Indic cases may be counted 1
/// instead of 2. Upgrade path: swap this body for the `unicode-width` crate
/// when dependency additions are permitted.
fn char_width(c: char) -> u16 {
    if c.is_control() {
        return 0;
    }
    let cp = c as u32;
    if in_ranges(cp, ZERO_WIDTH_RANGES) {
        return 0;
    }
    if in_ranges(cp, WIDE_RANGES) {
        return 2;
    }
    1
}

fn in_ranges(cp: u32, ranges: &[(u32, u32)]) -> bool {
    ranges.iter().any(|&(lo, hi)| cp >= lo && cp <= hi)
}

/// Combining marks, variation selectors and format characters (width 0).
const ZERO_WIDTH_RANGES: &[(u32, u32)] = &[
    (0x0300, 0x036F),
    (0x0483, 0x0489),
    (0x0591, 0x05BD),
    (0x05BF, 0x05BF),
    (0x05C1, 0x05C2),
    (0x05C4, 0x05C5),
    (0x05C7, 0x05C7),
    (0x0610, 0x061A),
    (0x064B, 0x065F),
    (0x0670, 0x0670),
    (0x06D6, 0x06DC),
    (0x06DF, 0x06E4),
    (0x06E7, 0x06E8),
    (0x06EA, 0x06ED),
    (0x0711, 0x0711),
    (0x0730, 0x074A),
    (0x07A6, 0x07B0),
    (0x07EB, 0x07F3),
    (0x0816, 0x0819),
    (0x081B, 0x0823),
    (0x0825, 0x0827),
    (0x0829, 0x082D),
    (0x0E31, 0x0E31),
    (0x0E34, 0x0E3A),
    (0x0E47, 0x0E4E),
    (0x1AB0, 0x1AFF),
    (0x1DC0, 0x1DFF),
    (0x200B, 0x200F),
    (0x202A, 0x202E),
    (0x2060, 0x2064),
    (0x20D0, 0x20FF),
    (0xFE00, 0xFE0F),
    (0xFE20, 0xFE2F),
    (0xFEFF, 0xFEFF),
];

/// East Asian Wide / Fullwidth and emoji scalars (width 2).
const WIDE_RANGES: &[(u32, u32)] = &[
    (0x1100, 0x115F),
    (0x2E80, 0x303E),
    (0x3041, 0x33FF),
    (0x3400, 0x4DBF),
    (0x4E00, 0x9FFF),
    (0xA000, 0xA4CF),
    (0xA960, 0xA97F),
    (0xAC00, 0xD7A3),
    (0xF900, 0xFAFF),
    (0xFE10, 0xFE19),
    (0xFE30, 0xFE6F),
    (0xFF00, 0xFF60),
    (0xFFE0, 0xFFE6),
    (0x16FE0, 0x16FE4),
    (0x17000, 0x18AFF),
    (0x1B000, 0x1B12F),
    (0x1F004, 0x1F004),
    (0x1F0CF, 0x1F0CF),
    (0x1F18E, 0x1F18E),
    (0x1F191, 0x1F19A),
    (0x1F200, 0x1F320),
    (0x1F32D, 0x1F335),
    (0x1F337, 0x1F37C),
    (0x1F37E, 0x1F393),
    (0x1F3A0, 0x1F3CA),
    (0x1F3CF, 0x1F3D3),
    (0x1F3E0, 0x1F3F0),
    (0x1F3F4, 0x1F3F4),
    (0x1F3F8, 0x1F43E),
    (0x1F440, 0x1F440),
    (0x1F442, 0x1F4FC),
    (0x1F4FF, 0x1F53D),
    (0x1F54B, 0x1F54E),
    (0x1F550, 0x1F567),
    (0x1F57A, 0x1F57A),
    (0x1F595, 0x1F596),
    (0x1F5A4, 0x1F5A4),
    (0x1F5FB, 0x1F64F),
    (0x1F680, 0x1F6C5),
    (0x1F6CC, 0x1F6CC),
    (0x1F6D0, 0x1F6D2),
    (0x1F6EB, 0x1F6EC),
    (0x1F6F4, 0x1F6FC),
    (0x1F7E0, 0x1F7EB),
    (0x1F90C, 0x1F93A),
    (0x1F93C, 0x1F945),
    (0x1F947, 0x1F9FF),
    (0x1FA70, 0x1FAFF),
    (0x20000, 0x2FFFD),
    (0x30000, 0x3FFFD),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Vertical pane border from the frozen harness set: a main-only column
    /// must never draw one.
    const BORDER: char = '│';

    fn line(s: &str) -> String {
        s.to_owned()
    }

    /// Display cells of a composed row.
    fn cells(row: &str) -> usize {
        row.chars()
            .map(|c| usize::from(char_width(c)))
            .sum::<usize>()
    }

    fn compose(cols: u16, rows: u16, center: &[String], right: &[String]) -> Vec<String> {
        compose_frame(&Layout::new(cols, rows), &[], center, right)
    }

    // Contract note: the former permanent three-pane shell (left `Sessions`
    // + center + right `Status`/`MCP`, thirds with `│` borders) is superseded
    // by upstream `session/index.tsx:255-278` (no permanent left all-sessions
    // pane; a fixed 42-column sidebar only when `cols > 120`) and by the
    // user's explicit layout-parity requirement. Expectations below are
    // re-pinned to that contract; unicode, control-stripping and resource
    // bound assertions are preserved.

    #[test]
    fn main_only_at_120_and_below_has_no_left_or_right_pane() {
        for cols in [0u16, 1, 10, 40, 60, 79, 80, 120] {
            let layout = Layout::new(cols, 24);
            assert_eq!(layout.left_width, 0, "{cols}: no left pane");
            assert_eq!(layout.right_width, 0, "{cols}: no sidebar at <= 120");
            assert_eq!(layout.center_width, cols, "{cols}: main spans viewport");
        }
        let layout = Layout::new(120, 30);
        assert!(!layout.compact, "120x30 is the normal main shell");
        assert_eq!(
            usize::from(layout.center_width) - 4,
            116,
            "logical content width after the view's 2+2 padding (SPEC matrix)"
        );
    }

    #[test]
    fn fixed_42_sidebar_above_120_never_thirds() {
        for (cols, center) in [
            (121u16, 79u16),
            (122, 80),
            (132, 90),
            (160, 118),
            (200, 158),
        ] {
            let layout = Layout::new(cols, 30);
            assert!(!layout.compact, "{cols} is the normal main shell");
            assert_eq!(layout.left_width, 0, "{cols}: no permanent left pane");
            assert_eq!(
                layout.right_width, SIDEBAR_WIDTH,
                "{cols}: sidebar is the fixed 42-column box"
            );
            assert_eq!(layout.center_width, center, "{cols}: center = cols - 42");
            assert_eq!(
                layout.left_width + layout.center_width + layout.right_width,
                layout.cols,
                "{cols}: fills the viewport with no separator cells"
            );
        }
        assert_eq!(usize::from(Layout::new(121, 30).center_width) - 4, 75);
        assert_eq!(usize::from(Layout::new(160, 30).center_width) - 4, 114);
    }

    #[test]
    fn short_viewport_suppresses_sidebar() {
        let layout = Layout::new(160, 1);
        assert_eq!(layout.right_width, 0, "sidebar needs enough height");
        assert_eq!(layout.center_width, 160, "main keeps the full width");
    }

    #[test]
    fn main_column_never_draws_left_or_right_or_borders() {
        let center = vec![
            line("workspace line alpha"),
            line("workspace line beta"),
            line("> prompt"),
        ];
        let right = vec![line("Status"), line("MCP"), line("idle")];
        let frame = compose(120, 30, &center, &right);
        assert_eq!(frame.len(), 30, "every viewport row is emitted");
        for (index, row) in frame.iter().enumerate() {
            assert_eq!(cells(row), 120, "row {index} must span 120 cells");
            assert!(!row.contains(BORDER), "row {index}: no pane separators");
            assert!(!row.contains("Status"), "row {index}: right pane hidden");
            assert!(!row.contains("MCP"), "row {index}: right pane hidden");
        }
        for (index, body) in center.iter().enumerate() {
            assert!(
                frame[index].contains(body.as_str()),
                "main row {index} retained"
            );
        }
    }

    #[test]
    fn left_content_is_never_painted() {
        let left = vec![line("OpenCode RK (native)"), line("Sessions")];
        let frame = compose_frame(&Layout::new(120, 30), &left, &[line("main")], &[]);
        for (index, row) in frame.iter().enumerate() {
            assert!(!row.contains("Sessions"), "row {index}: left leaked");
            assert!(
                !row.contains("OpenCode RK (native)"),
                "row {index}: left banner leaked"
            );
        }
    }

    #[test]
    fn wide_sidebar_is_concatenated_without_a_separator() {
        let frame = compose(
            160,
            3,
            &[line("main0"), line("main1"), line("main2")],
            &[line("side0"), line("side1"), line("side2")],
        );
        assert_eq!(frame.len(), 3);
        for (index, row) in frame.iter().enumerate() {
            assert_eq!(cells(row), 160, "row {index} spans 160 cells");
            assert!(!row.contains(BORDER), "row {index}: no separator");
        }
        assert!(frame[0].starts_with("main0"));
        let sidebar: String = frame[0].chars().skip(160 - 42).collect();
        assert!(sidebar.starts_with("side0"), "sidebar paints at cols - 42");
    }

    #[test]
    fn wide_and_combining_glyphs_keep_exact_cell_width() {
        let frame = compose(
            120,
            30,
            &[
                line("日本語テスト"),
                line("e\u{0301} combining"),
                line("\u{1F600} emoji"),
                line(&"漢".repeat(50)),
            ],
            &[],
        );
        for (index, row) in frame.iter().enumerate() {
            assert_eq!(
                cells(row),
                120,
                "row {index} must stay 120 cells with wide glyphs"
            );
        }
        assert!(frame[0].contains("日本語"), "wide glyphs are never sliced");
        assert!(
            frame[1].contains("e\u{0301}"),
            "combining marks stay attached to their kept base"
        );
        assert!(
            frame[1].contains("combining"),
            "zero-width marks behind a base keep alignment"
        );
        assert!(
            frame[3].contains('漢'),
            "wide text truncates on a cell boundary"
        );
    }

    #[test]
    fn control_and_ansi_bytes_never_reach_the_frame() {
        let frame = compose(
            160,
            30,
            &[line("line1\nline2\ttab\x1b]0;title\x07")],
            &[line("\x1b[31mSessions\x1b[0m\x07")],
        );
        for (index, row) in frame.iter().enumerate() {
            for c in row.chars() {
                assert!(!c.is_control(), "row {index} leaked control scalar {c:?}");
            }
            assert!(!row.contains('\x1b'), "row {index} leaked ESC");
        }
        assert!(
            frame[0].contains("line1 line2 tab"),
            "controls became spaces"
        );
        assert!(!frame[0].contains("31m"), "no CSI residue in the sidebar");
    }

    #[test]
    fn long_lines_are_bounded_to_the_pane() {
        let long = "x".repeat(10_000);
        let frame = compose(160, 30, &[line(&long)], &[line(&long)]);
        assert_eq!(frame.len(), 30);
        for (index, row) in frame.iter().enumerate() {
            assert_eq!(cells(row), 160, "row {index} width");
            assert!(row.chars().count() <= 160, "row {index} scalar bound");
        }
    }

    #[test]
    fn absurd_viewports_are_bounded() {
        let layout = Layout::new(u16::MAX, u16::MAX);
        let frame = compose_frame(&layout, &[], &[], &[]);
        let cap = std::cmp::max(1, MAX_FRAME_CELLS / usize::from(u16::MAX));
        assert!(
            frame.len() <= cap,
            "composed rows {} exceed the cell budget {cap}",
            frame.len()
        );
        for row in &frame {
            assert_eq!(cells(row), usize::from(u16::MAX));
        }
    }

    #[test]
    fn zero_and_tiny_viewports_stay_safe() {
        for (cols, rows) in [
            (0, 0),
            (0, 30),
            (120, 0),
            (1, 1),
            (10, 4),
            (80, 2),
            (80, 24),
        ] {
            let layout = Layout::new(cols, rows);
            let frame = compose_frame(&layout, &[line("Sessions")], &[line("> x")], &[line("MCP")]);
            assert!(
                frame.len() <= usize::from(rows),
                "{cols}x{rows} must not emit more rows than it has"
            );
            for row in &frame {
                assert_eq!(cells(row), usize::from(cols), "{cols}x{rows} row width");
            }
            assert_eq!(layout.left_width, 0, "{cols}x{rows}: no left pane");
        }
    }

    #[test]
    fn hand_built_layout_cannot_paint_out_of_range() {
        let layout = Layout {
            cols: 10,
            rows: 3,
            left_width: 0,
            center_width: 9,
            right_width: 9,
            compact: false,
        };
        let frame = compose_frame(
            &layout,
            &[line("aaaaaaaaaaaaaaaaaaaaaa")],
            &[line("bbbbbbbbbbbbbbbbbbbbbb")],
            &[line("cccccccccccccccccccccc")],
        );
        assert_eq!(frame.len(), 3);
        for row in &frame {
            assert_eq!(cells(row), 10, "normalized to the viewport width");
        }
    }

    // --- bounded scanning, cluster boundaries --------------------------

    #[test]
    fn unterminated_osc_stops_at_bounded_scan_prefix() {
        let mut payload = String::from("\x1b]0;");
        payload.push_str(&"A".repeat(10 * 1024 * 1024));
        payload.push_str("PAYLOAD-BEYOND-SCAN");
        let frame = compose(160, 30, &[payload], &[line("MCP")]);
        assert_eq!(frame.len(), 30);
        for (index, row) in frame.iter().enumerate() {
            assert_eq!(cells(row), 160, "row {index} width");
            assert!(!row.contains('\x1b'), "row {index} leaked ESC");
        }
        assert!(!frame[0].contains('A'), "OSC payload never renders");
        assert!(
            !frame[0].contains("PAYLOAD-BEYOND-SCAN"),
            "scan stopped at the bounded prefix, not at the input end"
        );
        assert!(frame[0].contains("MCP"), "sidebar content intact");
    }

    #[test]
    fn unterminated_dcs_and_csi_never_emit_raw() {
        let dcs = format!("\x1bPq{}TAIL", "B".repeat(8192));
        let csi = format!("\x1b[38;2;1;2;3{}", "9".repeat(8192));
        let frame = compose(160, 30, &[csi], &[dcs]);
        let row = &frame[0];
        assert_eq!(cells(row), 160);
        assert!(!row.contains('B'), "DCS payload never renders");
        assert!(!row.contains('9'), "CSI parameters never render");
        assert!(!row.contains('\x1b'), "raw escape bytes never render");
    }

    #[test]
    fn huge_combining_run_retains_bounded_bytes() {
        let mut text = String::from("e");
        text.push_str(&"\u{0301}".repeat(100_000));
        let fitted = fit(&text, 40);
        assert_eq!(cells(&fitted), 40);
        assert!(fitted.starts_with("e\u{0301}"), "base keeps its first mark");
        assert!(fitted.chars().count() <= 40 + MAX_CONTINUATIONS);
        assert!(
            fitted.len() <= 40 * MAX_CLUSTER_BYTES + 40,
            "retained bytes must stay bounded for a huge combining run"
        );
    }

    #[test]
    fn zwj_cluster_is_dropped_whole_at_a_pane_edge() {
        let joined = "\u{1F469}\u{200D}\u{1F4BB}";
        let edge = format!("{}{}", "漢".repeat(19), joined);
        let fitted = fit(&edge, 39);
        assert_eq!(cells(&fitted), 39);
        assert!(!fitted.contains('\u{200D}'), "no dangling ZWJ");
        assert!(
            !fitted.contains('\u{1F469}'),
            "overflow cluster dropped whole"
        );
        let fits = format!("{}{}", "漢".repeat(18), joined);
        let fitted2 = fit(&fits, 39);
        let expected = format!("{}\u{1F469}\u{200D}\u{1F4BB} ", "漢".repeat(18));
        assert_eq!(fitted2, expected);
        assert!(fitted2.contains("\u{1F469}\u{200D}\u{1F4BB}"));
    }

    #[test]
    fn variation_selector_stays_with_its_base_and_orphan_drops() {
        let heart = "\u{2764}\u{FE0F}";
        let fitted = fit(&format!("{}{}", "漢".repeat(18), heart), 39);
        assert!(fitted.contains(heart), "VS16 stays attached to its base");
        assert_eq!(cells(&fitted), 39);
        let orphan = fit("\u{FE0F}abc", 40);
        assert!(!orphan.starts_with('\u{FE0F}'), "orphan VS is dropped");
        assert!(orphan.starts_with("abc"));
        assert_eq!(cells(&orphan), 40);
    }

    #[test]
    fn flag_pair_kept_whole_half_flag_dropped() {
        let jp = "\u{1F1EF}\u{1F1F5}";
        let fitted = fit(&format!("{jp}ok"), 40);
        assert!(fitted.starts_with(jp), "RI pair renders as one flag");
        assert_eq!(
            fitted,
            format!("{jp}ok{}", " ".repeat(36)),
            "flag pair occupies exactly 2 cells, rest is literal ASCII padding"
        );
        assert_eq!(cells(&fitted), 40);
        let half = fit("\u{1F1EF}", 40);
        assert!(
            !half.contains('\u{1F1EF}'),
            "a lone regional indicator is dropped, never a half flag"
        );
        assert_eq!(cells(&half), 40);
    }
}
