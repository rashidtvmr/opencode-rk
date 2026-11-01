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
use std::collections::BTreeMap;

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
    // Tamil dependent signs and vowels are combining editor marks.
    (0x0B82, 0x0B83),
    (0x0BBE, 0x0BC2),
    (0x0BC6, 0x0BC8),
    (0x0BCA, 0x0BCD),
    (0x0BD7, 0x0BD7),
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

// ---------------------------------------------------------------------------
// Session-frame geometry contract (work package P03).
//
// Upstream pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`:
//
// * `packages/tui/src/ui/layout.ts:1-27` — the session width model:
//   `SESSION_SIDEBAR_WIDTH = 42`, `SESSION_TABS_COMPACT_WIDTH = 5`,
//   `SESSION_TABS_COMPACT_BREAKPOINT = 12`, `SESSION_SIDEBAR_MAX_WIDTH = 72`,
//   `SESSION_CONTENT_MIN_WIDTH = 44`, `SESSION_CONTENT_PREFERRED_WIDTH = 64`,
//   `SESSION_PANEL_SPLIT_MIN_WIDTH = 108`, `sessionTabsFitVertically`,
//   `clampSessionTabsWidth`, `clampSessionPaneWidth`.
// * `packages/tui/src/component/session-frame.tsx:107-140` — the right-pane
//   predicates: `wide = dimensions().width - verticalTabsWidth > 120`, the
//   child-session sidebar suppression, the `auto` config gate and the
//   `rightPane()` precedence `panel -> sidebar -> terminal -> sidebar`.
// * `packages/tui/src/component/session-frame.tsx:325-398` — the docked right
//   box is `SESSION_SIDEBAR_WIDTH` for the sidebar and `paneResize.size()`
//   for a terminal/panel; a non-wide sidebar is an absolute overlay instead.
// * `packages/tui/src/routes/session/index.tsx:260` — view content width is
//   `(width ?? terminalWidth - verticalTabsWidth) - 4`.
// * `packages/tui/src/routes/session/index.tsx:1511-1517` — the session column
//   pads left/right by `terminalWidth < 44 ? 1 : 2` and reserves
//   `paddingBottom={1}`.
// * `packages/tui/src/routes/session/index.tsx:1527-1537` — the transcript
//   scrollbox reserves `paddingRight` 1 plus a 1-column `paddingLeft` track
//   when `showScrollbar` is on.
// * `packages/tui/src/routes/session/index.tsx:1570-1589` — one `height={1}`
//   row below the transcript is always reserved for the jump/loading action.
// * `packages/tui/src/routes/session/sidebar.tsx:20-76` — the 42-column sidebar
//   pads 1/1/2/2, pins a `flexShrink={0}` title, scrolls the middle with an
//   ABSOLUTE 1-column scrollbar (`position: "absolute"`, `right: 0`) over the
//   content's reserved `paddingRight={1}`, and pins a footer.
//
// These are pure scalars/predicates; the rectangle model lives in
// `native_layout::SessionRegions` and the row composition in
// `native_shell_frame`. This module must stay self-contained because
// `tests/native_state_platform_alias.rs` compiles it through `#[path]`.
// ---------------------------------------------------------------------------

/// Fixed docked sidebar width (`ui/layout.ts:1`).
pub const SESSION_SIDEBAR_WIDTH: u16 = 42;
/// Narrowest rail width (`ui/layout.ts:2`).
pub const SESSION_TABS_COMPACT_WIDTH: u16 = 5;
/// A rail narrower than this renders in compact form
/// (`ui/layout.ts:3`, `component/session-tabs.tsx:562`).
pub const SESSION_TABS_COMPACT_BREAKPOINT: u16 = 12;
/// Widest rail the clamp will produce (`ui/layout.ts:4`).
pub const SESSION_SIDEBAR_MAX_WIDTH: u16 = 72;
/// Session content minimum (`ui/layout.ts:5`).
pub const SESSION_CONTENT_MIN_WIDTH: u16 = 44;
/// Session content preferred width (`ui/layout.ts:6`).
pub const SESSION_CONTENT_PREFERRED_WIDTH: u16 = 64;
/// Docked panel split minimum: preferred + minimum content
/// (`ui/layout.ts:9`).
pub const SESSION_PANEL_SPLIT_MIN_WIDTH: u16 = SESSION_CONTENT_PREFERRED_WIDTH + SESSION_CONTENT_MIN_WIDTH;
/// Horizontal inset total: 2 left + 2 right (`routes/session/index.tsx:260`).
pub const SESSION_CONTENT_PADDING: u16 = 4;
/// `wide` is a strict comparison against this width
/// (`component/session-frame.tsx:107`, `routes/session/index.tsx:257`).
pub const SESSION_WIDE_THRESHOLD: u16 = 120;
/// The narrow-terminal single-column inset (`routes/session/index.tsx:1515`).
pub const SESSION_NARROW_INSET: u16 = 1;
/// The normal left/right inset (`routes/session/index.tsx:1515-1516`).
pub const SESSION_WIDE_INSET: u16 = 2;
/// Terminal width below which the inset collapses to one column
/// (`routes/session/index.tsx:1515`).
pub const SESSION_NARROW_INSET_BREAKPOINT: u16 = 44;
/// Sidebar outer padding: `paddingTop`/`paddingBottom`
/// (`routes/session/sidebar.tsx:26-27`).
pub const SESSION_SIDEBAR_PAD_VERTICAL: u16 = 1;
/// Sidebar outer padding: `paddingLeft`/`paddingRight`
/// (`routes/session/sidebar.tsx:28-29`).
pub const SESSION_SIDEBAR_PAD_HORIZONTAL: u16 = 2;
/// Sidebar content reserves one right padding column that the ABSOLUTE
/// scrollbar overlays (`routes/session/sidebar.tsx:56-69`).
pub const SESSION_SIDEBAR_SCROLL_RESERVE: u16 = 1;
/// Transcript scrollbar cells: the viewport `paddingRight` (1) plus the
/// 1-column track (`routes/session/index.tsx:1527-1532`).
pub const SESSION_TRANSCRIPT_SCROLLBAR_CELLS: u16 = 2;
/// The always-present action row below the transcript
/// (`routes/session/index.tsx:1570`).
pub const SESSION_RESERVED_ACTION_ROWS: u16 = 1;

/// `sessionTabsFitVertically(total, width)` (`ui/layout.ts:11-13`): vertical
/// tabs fit only when the session keeps its preferred 64-column width beside
/// the rail.
#[must_use]
pub const fn session_tabs_fit_vertically(total: u16, width: u16) -> bool {
    total >= width.saturating_add(SESSION_CONTENT_PREFERRED_WIDTH)
}

/// `clampSessionTabsWidth(width, total)` (`ui/layout.ts:15-20`):
/// `max(5, min(width, 72, total - 44))`.
#[must_use]
pub fn clamp_session_tabs_width(width: u16, total: u16) -> u16 {
    let ceiling = width
        .min(SESSION_SIDEBAR_MAX_WIDTH)
        .min(total.saturating_sub(SESSION_CONTENT_MIN_WIDTH));
    ceiling.max(SESSION_TABS_COMPACT_WIDTH)
}

/// `clampSessionPaneWidth(width, total)` (`ui/layout.ts:22-26`):
/// `max(min(24, floor(total/2)), min(width, max(floor(total/2), total - 44)))`.
#[must_use]
pub fn clamp_session_pane_width(width: u16, total: u16) -> u16 {
    let half = 1.max(total / 2);
    let lower = 24.min(half);
    let upper = half.max(total.saturating_sub(SESSION_CONTENT_MIN_WIDTH));
    lower.max(width.min(upper))
}

/// A rail narrower than [`SESSION_TABS_COMPACT_BREAKPOINT`] renders compact
/// (`component/session-tabs.tsx:562-564`): stride 2 instead of 3.
#[must_use]
pub const fn session_tabs_compact(width: u16) -> bool {
    width < SESSION_TABS_COMPACT_BREAKPOINT
}

/// `wide = dimensions().width - verticalTabsWidth > 120`
/// (`component/session-frame.tsx:107`). Strict at exactly 120.
#[must_use]
pub const fn session_wide(width: u16, vertical_tabs_width: u16) -> bool {
    width.saturating_sub(vertical_tabs_width) > SESSION_WIDE_THRESHOLD
}

/// Configured sidebar mode (`config.session.sidebar`, default `auto`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SidebarMode {
    /// Visible automatically on wide viewports.
    Auto,
    /// Never visible automatically; only an explicit open shows it.
    Hide,
}

impl SidebarMode {
    /// Parse the persisted config value; anything other than `hide` is
    /// `auto`, matching `(config.data.session?.sidebar ?? "auto")`
    /// (`component/session-frame.tsx:112`).
    #[must_use]
    pub fn parse(value: &str) -> SidebarMode {
        if value == "hide" {
            SidebarMode::Hide
        } else {
            SidebarMode::Auto
        }
    }
}

/// `sidebarVisible()` (`component/session-frame.tsx:108-113`): child sessions
/// never show the sidebar; an explicit open always shows it; otherwise only
/// `auto` + `wide`.
#[must_use]
pub fn sidebar_visible(child_session: bool, sidebar_open: bool, mode: SidebarMode, wide: bool) -> bool {
    if child_session {
        return false;
    }
    if sidebar_open {
        return true;
    }
    mode == SidebarMode::Auto && wide
}

/// Which single pane occupies the right column
/// (`component/session-frame.tsx:114-120`, `rightPane()`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RightPane {
    None,
    Panel,
    Sidebar,
    Terminal,
}

/// `rightPane()` precedence (`component/session-frame.tsx:115-120`): an active
/// panel wins over everything, then an explicitly open (and visible) sidebar,
/// then a selected terminal, then the automatic sidebar. Exactly one pane is
/// returned, so no two right panes can be painted at once.
#[must_use]
pub fn right_pane(
    panel_open: bool,
    sidebar_open: bool,
    sidebar_visible: bool,
    terminal_selected: bool,
) -> RightPane {
    if panel_open {
        return RightPane::Panel;
    }
    if sidebar_open && sidebar_visible {
        return RightPane::Sidebar;
    }
    if terminal_selected {
        return RightPane::Terminal;
    }
    if sidebar_visible {
        return RightPane::Sidebar;
    }
    RightPane::None
}

/// Docked right-pane width (`component/session-frame.tsx:329-331`): the
/// sidebar is always [`SESSION_SIDEBAR_WIDTH`], a terminal/panel uses the
/// (clamped) pane-resize size, and no pane reserves nothing.
#[must_use]
pub const fn right_pane_width(pane: RightPane, pane_resize: u16) -> u16 {
    match pane {
        RightPane::None => 0,
        RightPane::Sidebar => SESSION_SIDEBAR_WIDTH,
        RightPane::Panel | RightPane::Terminal => pane_resize,
    }
}

/// Default pane width before any resize: `max(1, floor(available / 2))`
/// (`component/session-frame.tsx:44-45`, `defaultPaneWidth`).
#[must_use]
pub const fn default_pane_width(available: u16) -> u16 {
    let half = available / 2;
    if half == 0 {
        1
    } else {
        half
    }
}

/// Left/right inset of the session column: `width < 44 ? 1 : 2`
/// (`routes/session/index.tsx:1515-1516`). Note upstream keys this on the
/// TERMINAL width, not the session column width, so the caller must pass the
/// terminal width.
#[must_use]
pub const fn session_horizontal_inset(terminal_width: u16) -> u16 {
    if terminal_width < SESSION_NARROW_INSET_BREAKPOINT {
        SESSION_NARROW_INSET
    } else {
        SESSION_WIDE_INSET
    }
}

/// View content width: `(width ?? terminalWidth - verticalTabsWidth) - 4`
/// (`routes/session/index.tsx:260`). The `- 4` is the view's own 1+1 or 2+2
/// padding and is applied by the view, never folded into pane geometry.
#[must_use]
pub const fn session_content_width(session_width: u16) -> u16 {
    session_width.saturating_sub(SESSION_CONTENT_PADDING)
}

/// Cells the transcript loses while the scrollbar is shown
/// (`routes/session/index.tsx:1527-1532`): one viewport padding column plus
/// the one-column track. Hidden reserves nothing.
#[must_use]
pub const fn transcript_scrollbar_reservation(shown: bool) -> u16 {
    if shown {
        SESSION_TRANSCRIPT_SCROLLBAR_CELLS
    } else {
        0
    }
}

/// Cells the sidebar loses to its ABSOLUTE scrollbar
/// (`routes/session/sidebar.tsx:56-69`): zero, because the bar is painted over
/// the content's constant `paddingRight={1}`. Headings therefore never reflow
/// when the bar appears.
#[must_use]
pub const fn sidebar_scrollbar_reservation(_shown: bool) -> u16 {
    0
}

// ---------------------------------------------------------------------------
// Scroll anchors, tab switch and restore-after-reflow (LAY-17).
//
// Upstream pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`:
//
// * `routes/session/anchors.ts:4` — `AnchorTarget` is
//   `{type:"part", ref:{messageID,partID}}` or `{type:"group", groupID}`;
//   `:11-15` `anchorKey` canonicalises it and `:19-25` `entryRef` maps a whole
//   non-assistant message to the synthetic `"message"` body part; `:53-92`
//   `createTimelineAnchors` registers mounted rows with their content `y`,
//   drops destroyed/zero-height rows and lists them sorted by `y`.
// * `routes/session/index.tsx:542-571` — `saveScrollAnchor`: at the bottom the
//   stored anchor is CLEARED (sticky bottom); otherwise the deepest mounted
//   anchor whose `screenY = node.y - scroll.viewport.y` is at or above the
//   target's inset (1 for a group header or an assistant message, else 0) is
//   stored; with no mounted rows the previous anchor survives (unmount).
// * `routes/session/index.tsx:572-620` — `restoreScrollPosition`: no anchor
//   scrolls to the bottom; an anchor whose target no longer mounts clears the
//   anchor and scrolls to the bottom; otherwise
//   `target = scrollTop + node.y - scroll.viewport.y - anchor.screenY` and a
//   target past `scrollHeight - viewport.height` adds navigation slack
//   (`routes/session/message-navigation.ts:8-16`) so the semantic anchor keeps
//   its saved screen row.
// * `routes/session/index.tsx:409-417,1470-1488` — `holdAnchor`: a disclosure
//   toggle re-scrolls to `layoutTop(node) - held.top`, i.e. the same "keep the
//   toggled header on its viewport row through the reflow" rule.
// * `context/session-tabs.tsx:45-48` — `ScrollAnchor` is `{target, screenY}`;
//   `:395-411` the map is keyed by the route's session id (a child session
//   keeps its OWN anchor) but an entry is only valid while `root(sessionID)`
//   is an open tab, and an identical `(key, screenY)` pair is not rewritten;
//   `:350-355` closing a tab deletes the anchors of the whole session family.
//
// The model is a pure function of caller-owned rows plus an injectable reflow
// (no clock, no scrollbox, no rendering). It lives in this self-contained
// module so a width reflow and a tab switch restore through one arithmetic and
// `tests/native_state_platform_alias.rs` keeps compiling it standalone.
// ---------------------------------------------------------------------------

/// Vertical inset of a group header or an assistant message anchor
/// (`routes/session/index.tsx:557-561`): such a row counts as "at the top"
/// while it sits on the first row BELOW the scroll viewport's top edge.
pub const ANCHOR_THREAD_INSET: u16 = 1;

/// Identity of a scroll anchor target (`routes/session/anchors.ts:4`).
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum AnchorTarget {
    /// One rendered message part (`{type:"part", ref}`).
    Part {
        /// Owning message id.
        message_id: String,
        /// Part id inside that message.
        part_id: String,
    },
    /// One collapsible timeline group (`{type:"group", groupID}`).
    Group {
        /// Canonical group id (`anchors.ts:27-31`).
        group_id: String,
    },
}

impl AnchorTarget {
    /// A message-part target (`anchors.ts:19-25`).
    #[must_use]
    pub fn part(message_id: &str, part_id: &str) -> AnchorTarget {
        AnchorTarget::Part {
            message_id: message_id.to_owned(),
            part_id: part_id.to_owned(),
        }
    }

    /// A group target (`routes/session/anchor-view.tsx:39-53`).
    #[must_use]
    pub fn group(group_id: &str) -> AnchorTarget {
        AnchorTarget::Group {
            group_id: group_id.to_owned(),
        }
    }

    /// Canonical identity key (`anchors.ts:11-15`, `anchorKey`).
    ///
    /// Same shape as upstream's `JSON.stringify` tuple: `["part",msg,part]`
    /// or `["group",id]`. Only equality matters to the model, but keeping the
    /// upstream spelling makes saved anchors comparable with a port that
    /// persists them.
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            AnchorTarget::Part {
                message_id,
                part_id,
            } => format!("[\"part\",{message_id:?},{part_id:?}]"),
            AnchorTarget::Group { group_id } => format!("[\"group\",{group_id:?}]"),
        }
    }

    /// True for a timeline group header (`anchors.ts:4`).
    #[must_use]
    pub fn is_group(&self) -> bool {
        matches!(self, AnchorTarget::Group { .. })
    }
}

/// Default inset rule: a group header and an assistant message keep one row
/// under the viewport's top edge, every other anchor may sit on the edge
/// itself (`routes/session/index.tsx:557-561`). Callers that also know the
/// message role pass their own closure to [`capture_scroll_anchor`].
#[must_use]
pub fn default_anchor_inset(target: &AnchorTarget) -> u16 {
    if target.is_group() {
        ANCHOR_THREAD_INSET
    } else {
        0
    }
}

/// A saved viewport anchor (`context/session-tabs.tsx:45-48`):
/// `{target, screenY}`. `screen_y` is the row inside the scroll viewport the
/// semantic target must keep after a tab switch or a reflow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScrollAnchor {
    /// Semantic target.
    pub target: AnchorTarget,
    /// Rows from the top of the scroll viewport (0 = the viewport's first row).
    pub screen_y: u16,
}

/// One mounted anchor row: the semantic target plus its content `y`
/// (`anchors.ts:53-92`, already filtered to mounted, non-zero-height rows).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MountedAnchor {
    /// Semantic target.
    pub target: AnchorTarget,
    /// Row offset of the target's top inside the scroll content.
    pub y: u16,
}

/// `saveScrollAnchor()` (`routes/session/index.tsx:542-571`).
///
/// Sticky bottom wins: while the reader follows the bottom the anchor is
/// cleared (`None`). Otherwise the deepest mounted row that is still at or
/// above its inset is chosen (a timeline lists rows in `y` order, so that is
/// the last qualifying row and therefore the largest `screenY`); with nothing
/// qualifying the first mounted row is used. An empty list also yields `None`;
/// the only upstream case that keeps the stored anchor is an unmounting save
/// with no mounted children, which the caller skips before capturing
/// (`routes/session/index.tsx:546-547`).
#[must_use]
pub fn capture_scroll_anchor(
    mounted: &[MountedAnchor],
    scroll_top: u16,
    away_from_bottom: bool,
    inset: impl Fn(&AnchorTarget) -> u16,
) -> Option<ScrollAnchor> {
    if !away_from_bottom || mounted.is_empty() {
        return None;
    }
    // `anchors.list()` is sorted by `node.y` (`anchors.ts:55-60`); select
    // explicitly by `y` so the model never depends on registration order.
    let mut first: Option<usize> = None;
    let mut best: Option<usize> = None;
    for (index, row) in mounted.iter().enumerate() {
        if first.is_none_or(|current| row.y < mounted[current].y) {
            first = Some(index);
        }
        if row.y.saturating_sub(scroll_top) > inset(&row.target) {
            continue;
        }
        // Deepest qualifying row wins; ties keep the earliest registration,
        // matching the strict `>` in `saveScrollAnchor` (`index.tsx:566`).
        if best.is_none_or(|current| row.y > mounted[current].y) {
            best = Some(index);
        }
    }
    let index = best.or(first)?;
    Some(ScrollAnchor {
        target: mounted[index].target.clone(),
        screen_y: mounted[index].y.saturating_sub(scroll_top),
    })
}

/// Per-session saved anchors (`context/session-tabs.tsx:92,395-411`).
///
/// Keyed by the route's session id, so a child session keeps an anchor
/// independent of its root tab. Validity is checked against the ROOT tab
/// (`context/session-tabs.tsx:396-397,401-402`), and closing a tab removes the
/// anchors of the root plus all of its children (`:350-355`).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TabScrollAnchors {
    entries: BTreeMap<String, ScrollAnchor>,
}

impl TabScrollAnchors {
    /// No stored anchors.
    #[must_use]
    pub fn new() -> TabScrollAnchors {
        TabScrollAnchors {
            entries: BTreeMap::new(),
        }
    }

    /// Raw stored anchor (`sessionTabs.scrollAnchor(sessionID)`,
    /// `context/session-tabs.tsx:395-399`). Re-validation against the root tab
    /// happens when the entry is written ([`TabScrollAnchors::set`]), since
    /// this model does not own the tab list.
    #[must_use]
    pub fn get(&self, session_id: &str) -> Option<&ScrollAnchor> {
        self.entries.get(session_id)
    }

    /// `sessionTabs.setScrollAnchor(sessionID, anchor)`
    /// (`context/session-tabs.tsx:400-411`).
    ///
    /// `root_session_id` is `data.session.root(session_id)` and `tab_open`
    /// answers whether that root still has a tab. A `None` anchor or a closed
    /// root tab deletes the entry; an identical `(target key, screenY)` pair is
    /// not rewritten. Returns whether the stored map changed.
    pub fn set(
        &mut self,
        session_id: &str,
        root_session_id: &str,
        anchor: Option<ScrollAnchor>,
        tab_open: impl Fn(&str) -> bool,
    ) -> bool {
        match anchor {
            Some(anchor) if tab_open(root_session_id) => {
                if let Some(current) = self.entries.get(session_id) {
                    if current.target.key() == anchor.target.key()
                        && current.screen_y == anchor.screen_y
                    {
                        return false;
                    }
                }
                self.entries.insert(session_id.to_owned(), anchor);
                true
            }
            _ => self.entries.remove(session_id).is_some(),
        }
    }

    /// Forget one session's anchor (tab close, `context/session-tabs.tsx:353`).
    pub fn clear(&mut self, session_id: &str) -> bool {
        self.entries.remove(session_id).is_some()
    }

    /// Forget a closed tab's whole family — the root plus every child
    /// (`context/session-tabs.tsx:350-355`). Returns how many anchors were
    /// removed.
    pub fn close_family(&mut self, family: &[&str]) -> usize {
        family
            .iter()
            .filter(|id| self.entries.remove(**id).is_some())
            .count()
    }

    /// Stored anchors.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when nothing is stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Post-reflow geometry of the anchored row, produced by the injectable
/// reflow (the layout is a pure function of the wrap width, so the caller owns
/// the row measurement).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReflowedAnchor {
    /// Total scrollable content height after the reflow, in rows, EXCLUDING
    /// any navigation slack appended by a previous restore
    /// (`routes/session/message-navigation.ts:14` subtracts `currentSlack`
    /// before measuring, so the caller passes the un-slacked height).
    pub content_height: u16,
    /// Scroll viewport height after the reflow, in rows.
    pub viewport_height: u16,
    /// Content `y` of the anchored row after the reflow; `None` when the
    /// target no longer mounts.
    pub anchor_y: Option<u16>,
}

/// Result of `restoreScrollPosition()` (`routes/session/index.tsx:572-620`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScrollRestore {
    /// Scroll offset to apply before the slack exists.
    pub scroll_top: u16,
    /// Extra content rows the caller must append below the transcript so the
    /// saved screen row stays reachable
    /// (`routes/session/message-navigation.ts:8-16`); 0 when not needed.
    pub slack: u16,
    /// Scroll offset to apply once [`ScrollRestore::slack`] rows are part of
    /// the content — the position upstream settles on after its retry.
    pub settled_scroll_top: u16,
    /// True when the settled position is the bottom of the content.
    pub at_bottom: bool,
    /// True when the reader stays scrolled up from the bottom.
    pub away_from_bottom: bool,
    /// True when the stored anchor was dropped because its target vanished.
    pub cleared: bool,
}

impl ScrollRestore {
    /// Scroll offset that puts the anchor back on its saved screen row: the
    /// settled position. For a slack-free restore it equals
    /// [`ScrollRestore::scroll_top`].
    #[must_use]
    pub fn desired_scroll_top(&self) -> u16 {
        self.settled_scroll_top
    }
}

/// `restoreScrollPosition()` (`routes/session/index.tsx:572-620`).
///
/// * no anchor -> stick to the bottom (`scrollTop = scrollHeight - viewport`);
/// * a target that no longer mounts -> `cleared`, then stick to the bottom;
/// * otherwise `target = anchor_y - screen_y`, and a target past the maximum
///   scroll offset keeps the row reachable through [`ScrollRestore::slack`].
///
/// The screen-row invariant the caller can assert is
/// `anchor_y - desired_scroll_top() == screen_y` for every mounted anchor.
#[must_use]
pub fn restore_scroll(anchor: Option<&ScrollAnchor>, reflowed: ReflowedAnchor) -> ScrollRestore {
    let content = i32::from(reflowed.content_height);
    let viewport = i32::from(reflowed.viewport_height);
    let maximum = content - viewport;
    let bottom = ScrollRestore {
        scroll_top: u16::try_from(maximum.max(0)).unwrap_or(u16::MAX),
        slack: 0,
        settled_scroll_top: u16::try_from(maximum.max(0)).unwrap_or(u16::MAX),
        at_bottom: true,
        away_from_bottom: false,
        cleared: false,
    };
    let Some(anchor) = anchor else {
        return bottom;
    };
    let Some(anchor_y) = reflowed.anchor_y else {
        return ScrollRestore {
            cleared: true,
            ..bottom
        };
    };
    let target = i32::from(anchor_y) - i32::from(anchor.screen_y);
    // `messageNavigationSlack` (`message-navigation.ts:14-15`):
    // `max(0, ceil(top + viewportHeight - contentHeight))`.
    let slack = (target + viewport - content).max(0);
    let settled = u16::try_from(target.max(0)).unwrap_or(u16::MAX);
    // Before the slack is appended only `max(0, maximum)` is reachable; then
    // the retry scrolls to the target itself.
    let scroll_top = if target > maximum {
        u16::try_from(maximum.max(0)).unwrap_or(u16::MAX)
    } else {
        settled
    };
    let limit = maximum + slack;
    let at_bottom = i32::from(settled) >= limit;
    ScrollRestore {
        scroll_top,
        slack: u16::try_from(slack).unwrap_or(u16::MAX),
        settled_scroll_top: settled,
        at_bottom,
        away_from_bottom: !at_bottom,
        cleared: false,
    }
}

/// Restore one session's scroll through the injected reflow.
///
/// `reflow` receives the saved target (or `None` while no anchor is stored)
/// and returns the geometry at the CURRENT width, so a tab switch and a
/// terminal resize both go through `restore_scroll`. A cleared anchor is
/// removed from `anchors`, matching `sessionTabs.setScrollAnchor(id, undefined)`
/// (`routes/session/index.tsx:587-591`).
pub fn restore_tab_scroll(
    anchors: &mut TabScrollAnchors,
    session_id: &str,
    reflow: impl FnOnce(Option<&AnchorTarget>) -> ReflowedAnchor,
) -> ScrollRestore {
    let stored = anchors.get(session_id).cloned();
    let reflowed = reflow(stored.as_ref().map(|anchor| &anchor.target));
    let restore = restore_scroll(stored.as_ref(), reflowed);
    if restore.cleared {
        anchors.clear(session_id);
    }
    restore
}

/// Injectable reflow for tests and for callers that own only row text: each
/// anchor row wraps to `ceil(cells / width)` rows using the same display-cell
/// measure [`fit`] uses. Narrowing the width therefore makes rows taller, moves
/// every later anchor down and changes the required `scrollTop` while the saved
/// screen row stays constant.
#[derive(Clone, Debug, Default)]
pub struct ReflowTimeline {
    rows: Vec<(AnchorTarget, String)>,
}

impl ReflowTimeline {
    /// A timeline from `(target, text)` rows in visual order.
    #[must_use]
    pub fn new(rows: Vec<(AnchorTarget, String)>) -> ReflowTimeline {
        ReflowTimeline { rows }
    }

    /// Append one row.
    pub fn push(&mut self, target: AnchorTarget, text: impl Into<String>) {
        self.rows.push((target, text.into()));
    }

    /// Wrapped row height of `text` at `width` display cells (at least 1).
    #[must_use]
    pub fn row_height(text: &str, width: u16) -> u16 {
        if width == 0 {
            return 1;
        }
        let cells = visit_unpadded_cells(text, usize::MAX, |_, _| {});
        let width = usize::from(width);
        let height = cells.div_ceil(width);
        u16::try_from(height.max(1)).unwrap_or(u16::MAX)
    }

    /// Content `y` of `target` at `width`, or `None` when it is not in the
    /// timeline (an unmounted target).
    #[must_use]
    pub fn content_y(&self, target: &AnchorTarget, width: u16) -> Option<u16> {
        let mut y: u16 = 0;
        for (row, text) in &self.rows {
            if row == target {
                return Some(y);
            }
            y = y.saturating_add(ReflowTimeline::row_height(text, width));
        }
        None
    }

    /// Total content height at `width`.
    #[must_use]
    pub fn content_height(&self, width: u16) -> u16 {
        self.rows.iter().fold(0u16, |y, (_, text)| {
            y.saturating_add(ReflowTimeline::row_height(text, width))
        })
    }

    /// Geometry for `target` at `width` (`viewport_height` rows visible).
    #[must_use]
    pub fn geometry(
        &self,
        target: &AnchorTarget,
        width: u16,
        viewport_height: u16,
    ) -> ReflowedAnchor {
        ReflowedAnchor {
            content_height: self.content_height(width),
            viewport_height,
            anchor_y: self.content_y(target, width),
        }
    }
}

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
    fn tamil_dependent_signs_share_painter_cells_with_their_base() {
        let text = "தமிழ்";
        let mut cells = Vec::new();
        visit_unpadded_cells(text, 20, |glyph, width| cells.push((glyph.to_owned(), width)));
        assert_eq!(cells, vec![("த".to_owned(), 1), ("மி".to_owned(), 1), ("ழ்".to_owned(), 1)]);
        assert_eq!(fit(text, 3), text);
        assert_eq!(fit("\u{0bbe}த", 3), "த  ");
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

    // -----------------------------------------------------------------------
    // LAY-02..LAY-07, LAY-11: session width model (P03).
    // -----------------------------------------------------------------------

    /// LAY-02. `routes/session/index.tsx:1515-1516`: the route inset flips from
    /// 1 to 2 at terminal width 44 (strict `< 44`), and the content width is
    /// the session column minus the 4 inset cells (`index.tsx:260`).
    #[test]
    fn lay_02_route_horizontal_inset_switches_between_43_and_44() {
        for (width, inset) in [(1u16, 1u16), (43, 1), (44, 2), (120, 2), (200, 2)] {
            assert_eq!(
                session_horizontal_inset(width),
                inset,
                "{width}: terminalWidth < 44 ? 1 : 2"
            );
            let painted = width.saturating_sub(inset * 2);
            assert!(
                painted <= width,
                "{width}: prompt/footer content stays inside the viewport"
            );
        }
        // Painted prompt/footer width: 43-2 = 41, then 44-4 = 40 (the inset
        // costs one more column each side even though the terminal grew).
        assert_eq!(43 - session_horizontal_inset(43) * 2, 41);
        assert_eq!(44 - session_horizontal_inset(44) * 2, 40);
        // The view's own content model always subtracts 4 (index.tsx:260) —
        // `contentWidth` uses the constant, not the width-dependent inset.
        assert_eq!(session_content_width(43), 39);
        assert_eq!(session_content_width(44), 40);
        assert_eq!(session_content_width(116), 112, "120-wide docked column - 4");
        assert_eq!(session_content_width(0), 0, "tiny viewport saturates");
    }
    /// LAY-01. The root frame fills the terminal at the checklist sizes
    /// (`routes/session/index.tsx:1510-1517`): exactly `rows` rows, each
    /// exactly `cols` display cells, and pane widths tile `cols` with no
    /// separator cell. Above 120 cols the fixed 42-col sidebar is the only
    /// extra pane; 80/120 stay main-only.
    #[test]
    fn lay_01_root_fills_reference_viewports() {
        for (cols, rows) in [(80u16, 24u16), (120, 40), (160, 48), (200, 60)] {
            let layout = Layout::new(cols, rows);
            let frame = compose_frame(&layout, &[], &[line("body")], &[line("side")]);
            assert_eq!(frame.len(), usize::from(rows), "{cols}x{rows}: every row");
            for (index, row) in frame.iter().enumerate() {
                assert_eq!(cells(row), usize::from(cols), "{cols}x{rows} row {index}");
            }
            assert_eq!(
                layout.left_width + layout.center_width + layout.right_width,
                cols,
                "{cols}x{rows}: panes tile the viewport"
            );
            let expected_sidebar = u16::from(cols > 120) * SIDEBAR_WIDTH;
            assert_eq!(layout.right_width, expected_sidebar, "{cols}x{rows}: sidebar");
        }
    }


    /// LAY-03 and LAY-05. `ui/layout.ts:11-13`: vertical tabs fit when
    /// `total >= rail + 64`. Default rail 42 crosses at 105/106, rail 60 at
    /// 123/124, narrow rail 5 at 68/69.
    #[test]
    fn lay_03_lay_05_vertical_eligibility_thresholds() {
        for (rail, below, above) in [(42u16, 105u16, 106u16), (60, 123, 124), (5, 68, 69)] {
            assert!(
                !session_tabs_fit_vertically(below, rail),
                "rail {rail}: width {below} (< {rail} + 64) stays horizontal"
            );
            assert!(
                session_tabs_fit_vertically(above, rail),
                "rail {rail}: width {above} (>= {rail} + 64) goes vertical"
            );
        }
        assert_eq!(SESSION_PANEL_SPLIT_MIN_WIDTH, 108, "64 + 44");
    }

    /// LAY-04. `ui/layout.ts:15-20` clamp bounds 5/72/min-content 44 and the
    /// compact transition at width 11/12 (`session-tabs.tsx:562`).
    #[test]
    fn lay_04_rail_clamp_and_compact_breakpoint() {
        assert_eq!(clamp_session_tabs_width(1, 200), SESSION_TABS_COMPACT_WIDTH);
        assert_eq!(clamp_session_tabs_width(10, 200), 10);
        assert_eq!(clamp_session_tabs_width(100, 200), SESSION_SIDEBAR_MAX_WIDTH);
        // total - 44 wins when the viewport is narrow: 60 - 44 = 16.
        assert_eq!(clamp_session_tabs_width(40, 60), 16);
        // Below the content minimum the ceiling saturates at the compact floor.
        assert_eq!(clamp_session_tabs_width(40, 30), SESSION_TABS_COMPACT_WIDTH);
        assert!(session_tabs_compact(11), "11 < 12 renders compact");
        assert!(!session_tabs_compact(12), "12 is the first full rail width");
    }

    /// LAY-06. `component/session-frame.tsx:107-113`: with no vertical rail the
    /// auto sidebar appears strictly above 120 (120 -> hidden, 121 -> 42 wide).
    #[test]
    fn lay_06_auto_sidebar_threshold_without_vertical_rail() {
        assert!(!session_wide(120, 0), "120 is not wide (strict >)");
        assert!(session_wide(121, 0), "121 is the first wide width");
        assert!(!sidebar_visible(false, false, SidebarMode::Auto, session_wide(120, 0)));
        assert!(sidebar_visible(false, false, SidebarMode::Auto, session_wide(121, 0)));
        assert_eq!(SESSION_SIDEBAR_WIDTH, 42, "auto sidebar docks at 42 columns");
        // Explicit open and config "hide" both override the auto rule.
        assert!(sidebar_visible(false, true, SidebarMode::Hide, false));
        assert!(!sidebar_visible(false, false, SidebarMode::Hide, true));
        assert!(!sidebar_visible(true, true, SidebarMode::Auto, true), "child sessions never");
        assert_eq!(SidebarMode::parse("hide"), SidebarMode::Hide);
        assert_eq!(SidebarMode::parse("auto"), SidebarMode::Auto);
        assert_eq!(SidebarMode::parse(""), SidebarMode::Auto);
    }

    /// LAY-07. `component/session-frame.tsx:107`: the threshold subtracts the
    /// default 42-column rail, so 162/163 is the crossing (162-42 = 120).
    #[test]
    fn lay_07_auto_sidebar_threshold_with_default_vertical_rail() {
        assert!(!session_wide(162, SESSION_SIDEBAR_WIDTH));
        assert!(session_wide(163, SESSION_SIDEBAR_WIDTH));
        assert!(!sidebar_visible(false, false, SidebarMode::Auto, session_wide(162, 42)));
        assert!(sidebar_visible(false, false, SidebarMode::Auto, session_wide(163, 42)));
        // The same 120-column available area is not wide however it was reached.
        assert!(!session_wide(120, 0));
        assert!(!session_wide(162, 42));
        // Tiny viewports saturate rather than wrap.
        assert!(!session_wide(0, 0));
        assert!(!session_wide(30, 42));
    }

    /// LAY-11. `component/session-frame.tsx:115-120` precedence: panel, then an
    /// explicitly open visible sidebar, then a terminal, then the automatic
    /// sidebar; exactly one pane result exists for every input.
    #[test]
    fn lay_11_right_pane_precedence_is_single_valued() {
        assert_eq!(right_pane(true, true, true, true), RightPane::Panel);
        assert_eq!(right_pane(false, true, true, true), RightPane::Sidebar);
        assert_eq!(right_pane(false, false, true, true), RightPane::Terminal);
        assert_eq!(right_pane(false, false, true, false), RightPane::Sidebar);
        assert_eq!(right_pane(false, false, false, false), RightPane::None);
        // An explicitly open but invisible (child session) sidebar falls through.
        assert_eq!(right_pane(false, true, false, true), RightPane::Terminal);
        assert_eq!(right_pane(false, true, false, false), RightPane::None);
        // Dock widths: sidebar is always 42, terminal/panel use the resize size.
        assert_eq!(right_pane_width(RightPane::Sidebar, 37), 42);
        assert_eq!(right_pane_width(RightPane::Terminal, 37), 37);
        assert_eq!(right_pane_width(RightPane::Panel, 37), 37);
        assert_eq!(right_pane_width(RightPane::None, 37), 0);
        assert_eq!(default_pane_width(120), 60);
        assert_eq!(default_pane_width(1), 1);
        assert_eq!(default_pane_width(0), 1);
    }

    /// LAY-16. Scrollbar reservations: the transcript loses the viewport
    /// padding plus the track (2 cells) while shown and nothing while hidden
    /// (`index.tsx:1527-1532`); the sidebar's absolute bar costs 0 columns
    /// because the content already reserves `paddingRight={1}`
    /// (`sidebar.tsx:56-69`).
    #[test]
    fn lay_16_scrollbar_reservations() {
        assert_eq!(transcript_scrollbar_reservation(true), 2);
        assert_eq!(transcript_scrollbar_reservation(false), 0);
        assert_eq!(sidebar_scrollbar_reservation(true), 0);
        assert_eq!(sidebar_scrollbar_reservation(false), 0);
        assert_eq!(SESSION_SIDEBAR_SCROLL_RESERVE, 1);
        // The sidebar content width is a function of the 42-column box only,
        // so headings cannot reflow when the bar toggles.
        let content = SESSION_SIDEBAR_WIDTH - SESSION_SIDEBAR_PAD_HORIZONTAL * 2;
        assert_eq!(content, 38);
        assert_eq!(content - SESSION_SIDEBAR_SCROLL_RESERVE, 37);
    }

    /// LAY-15. `index.tsx:1570` always reserves one row below the transcript,
    /// independent of the jump action's state.
    #[test]
    fn lay_15_reserved_action_row_is_constant() {
        assert_eq!(SESSION_RESERVED_ACTION_ROWS, 1);
    }

    // -----------------------------------------------------------------------
    // LAY-17: scroll anchor, tab switch and restore after reflow.
    // -----------------------------------------------------------------------

    fn timeline() -> ReflowTimeline {
        ReflowTimeline::new(vec![
            (AnchorTarget::part("m1", "p1"), "alpha ".repeat(30)),
            (AnchorTarget::group("g1"), "beta ".repeat(40)),
            (AnchorTarget::part("m2", "p2"), "gamma ".repeat(20)),
        ])
    }

    /// LAY-17. `anchors.ts:11-15`: the semantic key is the upstream JSON tuple,
    /// so the same part/group identity always maps to the same anchor.
    #[test]
    fn lay17_anchor_key_is_the_upstream_tuple() {
        assert_eq!(
            AnchorTarget::part("m1", "p1").key(),
            "[\"part\",\"m1\",\"p1\"]"
        );
        assert_eq!(AnchorTarget::group("g1").key(), "[\"group\",\"g1\"]");
        assert_ne!(
            AnchorTarget::part("m1", "p1").key(),
            AnchorTarget::part("m1", "p2").key(),
            "part identity includes the part id"
        );
        assert_eq!(default_anchor_inset(&AnchorTarget::group("g")), 1);
        assert_eq!(default_anchor_inset(&AnchorTarget::part("m", "p")), 0);
        assert!(AnchorTarget::group("g").is_group());
        assert!(!AnchorTarget::part("m", "p").is_group());
    }

    /// LAY-17. `index.tsx:542-571`: sticky bottom clears the anchor, the
    /// deepest row at or above its inset wins (a group header keeps the row
    /// below the viewport edge), and with nothing qualifying the first mounted
    /// row is stored instead.
    #[test]
    fn lay17_capture_prefers_deepest_visible_row_then_first() {
        let mounted = [
            MountedAnchor {
                target: AnchorTarget::part("m1", "p1"),
                y: 0,
            },
            MountedAnchor {
                target: AnchorTarget::group("g1"),
                y: 4,
            },
            MountedAnchor {
                target: AnchorTarget::part("m2", "p2"),
                y: 9,
            },
        ];
        let anchor = capture_scroll_anchor(&mounted, 3, true, default_anchor_inset)
            .expect("a scrolled-up transcript stores an anchor");
        assert_eq!(anchor.target, AnchorTarget::group("g1"));
        assert_eq!(anchor.screen_y, 1, "the group header sits one row down");
        // The same rows from the top: the part on row 0 wins at inset 0.
        let anchor = capture_scroll_anchor(&mounted, 0, true, default_anchor_inset).unwrap();
        assert_eq!(anchor.target, AnchorTarget::part("m1", "p1"));
        assert_eq!(anchor.screen_y, 0);
        // At the bottom the anchor is cleared (sticky bottom).
        assert_eq!(
            capture_scroll_anchor(&mounted, 3, false, default_anchor_inset),
            None
        );
        assert_eq!(capture_scroll_anchor(&[], 0, true, default_anchor_inset), None);
        // No row starts at or above the inset (older rows unmounted): the
        // first mounted row is stored instead of losing the position.
        let tail = [
            MountedAnchor {
                target: AnchorTarget::part("m9", "p9"),
                y: 3,
            },
            MountedAnchor {
                target: AnchorTarget::part("m10", "p10"),
                y: 7,
            },
        ];
        let anchor = capture_scroll_anchor(&tail, 0, true, default_anchor_inset).unwrap();
        assert_eq!(anchor.target, AnchorTarget::part("m9", "p9"));
        assert_eq!(anchor.screen_y, 3);
    }

    /// LAY-17. Switching tabs and narrowing the viewport reflows every row;
    /// the stored semantic anchor is restored to the SAME screen row even
    /// though the pixel offset changes, and the anchor is not rewritten when
    /// re-captured after the reflow.
    #[test]
    fn lay17_tab_switch_and_reflow_keep_anchor_and_screen_y() {
        let timeline = timeline();
        let mut anchors = TabScrollAnchors::new();

        // Wide layout: group header lands on screen row 1 after scrolling.
        let mounted = [
            MountedAnchor {
                target: AnchorTarget::part("m1", "p1"),
                y: 0,
            },
            MountedAnchor {
                target: AnchorTarget::group("g1"),
                y: 5,
            },
            MountedAnchor {
                target: AnchorTarget::part("m2", "p2"),
                y: 10,
            },
        ];
        let saved = capture_scroll_anchor(&mounted, 4, true, default_anchor_inset).unwrap();
        assert_eq!(saved.target, AnchorTarget::group("g1"));
        assert_eq!(saved.screen_y, 1);
        assert!(anchors.set("s1", "s1", Some(saved.clone()), |id| id == "s1"));

        // Reflow to a narrow viewport: rows wrap taller, the anchor moves from
        // content row 5 to content row 9.
        assert_eq!(timeline.content_y(&saved.target, 20), Some(9));
        assert_eq!(timeline.content_height(20), 25);
        let restored = restore_tab_scroll(&mut anchors, "s1", |target| {
            timeline.geometry(target.expect("stored anchor"), 20, 8)
        });
        assert_eq!(restored.scroll_top, 8, "9 - screen row 1");
        assert_eq!(restored.slack, 0);
        assert!(!restored.at_bottom && restored.away_from_bottom);
        assert!(!restored.cleared);
        assert_eq!(
            timeline.content_y(&saved.target, 20).unwrap() - restored.desired_scroll_top(),
            saved.screen_y,
            "the semantic anchor kept its screen row"
        );
        assert!(anchors.get("s1").is_some(), "the anchor survives the reflow");

        // Re-capturing at the new width yields the identical anchor, so the
        // tab-switch save is a no-op rather than drifting a row each time.
        let mounted_after = [
            MountedAnchor {
                target: AnchorTarget::part("m1", "p1"),
                y: 0,
            },
            MountedAnchor {
                target: AnchorTarget::group("g1"),
                y: 9,
            },
            MountedAnchor {
                target: AnchorTarget::part("m2", "p2"),
                y: 19,
            },
        ];
        let again =
            capture_scroll_anchor(&mounted_after, restored.settled_scroll_top, true, default_anchor_inset)
                .unwrap();
        assert_eq!(again, saved);
        assert!(
            !anchors.set("s1", "s1", Some(again), |id| id == "s1"),
            "an identical (target, screenY) pair is not rewritten"
        );

        // A second tab keeps its own anchor; restoring it leaves tab 1 alone.
        let other = capture_scroll_anchor(&mounted_after, 19, true, default_anchor_inset).unwrap();
        assert_eq!(other.target, AnchorTarget::part("m2", "p2"));
        assert!(anchors.set("s2", "s2", Some(other), |id| id == "s1" || id == "s2"));
        let first_before = anchors.get("s1").cloned();
        let second = restore_tab_scroll(&mut anchors, "s2", |target| {
            timeline.geometry(target.expect("stored anchor"), 40, 8)
        });
        assert_eq!(
            timeline.content_y(&AnchorTarget::part("m2", "p2"), 40),
            Some(10)
        );
        assert_eq!(second.settled_scroll_top, 10, "10 - screen row 0");
        assert_eq!(second.slack, 5, "the row must stay reachable: taller slack");
        assert!(second.at_bottom, "after the slack the row is the last content");
        assert_eq!(
            timeline.content_y(&AnchorTarget::part("m2", "p2"), 40).unwrap()
                - second.desired_scroll_top(),
            0,
            "screen row preserved through the slack"
        );
        assert_eq!(anchors.get("s1").cloned(), first_before, "tabs stay independent");
    }

    /// LAY-17. `index.tsx:584-592`: a target that no longer mounts clears the
    /// stored anchor and sticks to the bottom; a tab with no anchor starts at
    /// the bottom without disturbing any other tab.
    #[test]
    fn lay17_vanished_anchor_clears_and_sticks_to_bottom() {
        let mut anchors = TabScrollAnchors::new();
        anchors.set(
            "s1",
            "s1",
            Some(ScrollAnchor {
                target: AnchorTarget::part("gone", "p0"),
                screen_y: 0,
            }),
            |_| true,
        );
        let restore = restore_tab_scroll(&mut anchors, "s1", |_| ReflowedAnchor {
            content_height: 30,
            viewport_height: 10,
            anchor_y: None,
        });
        assert!(restore.cleared);
        assert!(restore.at_bottom && !restore.away_from_bottom);
        assert_eq!(restore.scroll_top, 20, "scrollHeight - viewport");
        assert_eq!(restore.desired_scroll_top(), 20);
        assert!(anchors.get("s1").is_none(), "the stale anchor is dropped");

        // Another tab is untouched by the fallback.
        anchors.set(
            "s2",
            "s2",
            Some(ScrollAnchor {
                target: AnchorTarget::group("g1"),
                screen_y: 1,
            }),
            |_| true,
        );
        let restore = restore_tab_scroll(&mut anchors, "s3", |_| ReflowedAnchor {
            content_height: 12,
            viewport_height: 12,
            anchor_y: None,
        });
        assert!(!restore.cleared);
        assert!(restore.at_bottom);
        assert_eq!(restore.scroll_top, 0, "content fits the viewport");
        assert!(anchors.get("s2").is_some());
    }

    /// LAY-17. `context/session-tabs.tsx:395-411`: a child session stores its
    /// own anchor keyed by its own id, validity follows the root tab, and
    /// closing a tab removes the whole family.
    #[test]
    fn lay17_child_anchor_is_independent_but_dies_with_the_root_tab() {
        let mut anchors = TabScrollAnchors::new();
        let child = ScrollAnchor {
            target: AnchorTarget::part("m-child", "p1"),
            screen_y: 2,
        };
        let root = ScrollAnchor {
            target: AnchorTarget::part("m-root", "p1"),
            screen_y: 0,
        };
        assert!(anchors.set("child", "root", Some(child.clone()), |id| id == "root"));
        assert!(anchors.set("root", "root", Some(root.clone()), |id| id == "root"));
        assert_eq!(anchors.get("child"), Some(&child));
        assert_eq!(anchors.get("root"), Some(&root));
        assert_eq!(anchors.len(), 2, "the child anchor is independent");

        // Closing the root tab invalidates child writes as well.
        assert!(!anchors.set("child2", "root", Some(child.clone()), |_| false));
        assert!(anchors.get("child2").is_none());

        assert_eq!(anchors.close_family(&["root", "child", "grandchild"]), 2);
        assert!(anchors.is_empty(), "the family's anchors are gone");
        // A cleared value is accepted and reported as a change exactly once.
        assert!(anchors.set("root", "root", Some(root.clone()), |_| true));
        assert!(anchors.set("root", "root", None, |_| true));
        assert!(!anchors.set("root", "root", None, |_| true));
    }

    /// LAY-17. `message-navigation.ts:8-16`: when the anchor is the last
    /// content, the restore appends slack so the saved screen row is still
    /// reachable instead of silently clamping the reader to a different row.
    #[test]
    fn lay17_restore_adds_slack_to_reach_the_saved_screen_row() {
        let timeline = ReflowTimeline::new(vec![
            (AnchorTarget::part("m1", "p1"), "a".repeat(10)),
            (AnchorTarget::part("m2", "p2"), "b".repeat(240)),
        ]);
        let target = AnchorTarget::part("m2", "p2");
        let geometry = timeline.geometry(&target, 80, 8);
        assert_eq!(geometry.content_height, 4, "content shorter than the viewport");
        assert_eq!(geometry.anchor_y, Some(1));
        let anchor = ScrollAnchor {
            target: target.clone(),
            screen_y: 0,
        };
        let restore = restore_scroll(Some(&anchor), geometry);
        assert_eq!(restore.scroll_top, 0, "nothing scrollable before the slack");
        assert_eq!(restore.slack, 5, "top + viewport - contentHeight");
        assert_eq!(restore.desired_scroll_top(), 1);
        assert_eq!(
            u16::from(geometry.content_height + restore.slack),
            9,
            "the slack makes the saved row reachable"
        );
        assert!(restore.at_bottom && !restore.away_from_bottom);
        // The screen-row invariant holds for every mounted anchor.
        assert_eq!(
            geometry.anchor_y.unwrap() - restore.desired_scroll_top(),
            anchor.screen_y
        );
    }

    /// LAY-17. Reflow measurement: heights come from the same display-cell
    /// measure as [`fit`], so wide glyphs and empty rows reflow like content.
    #[test]
    fn lay17_reflow_timeline_measures_wrapped_heights() {
        assert_eq!(ReflowTimeline::row_height("abc", 3), 1);
        assert_eq!(ReflowTimeline::row_height("abcd", 3), 2);
        assert_eq!(ReflowTimeline::row_height("", 4), 1);
        assert_eq!(
            ReflowTimeline::row_height("\u{6F22}\u{6F22}", 2),
            2,
            "two wide glyphs take two rows at width 2"
        );
        assert_eq!(ReflowTimeline::row_height("abc", 0), 1, "zero width saturates");
        let timeline = timeline();
        assert_eq!(timeline.content_y(&AnchorTarget::group("g1"), 40), Some(5));
        assert_eq!(timeline.content_y(&AnchorTarget::group("g1"), 20), Some(9));
        assert_eq!(timeline.content_height(40), 13);
        assert_eq!(timeline.content_height(20), 25);
        assert_eq!(
            timeline.content_y(&AnchorTarget::part("absent", "p"), 40),
            None,
            "an unmounted target has no content y"
        );
    }
}
