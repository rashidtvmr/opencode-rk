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
/// Max base scalars per cluster, bounding ZWJ join chains. Eight keeps every
/// RGI family (at most four bases) and a five-member chain in one cluster while
/// still splitting an arbitrarily long chain into further bounded clusters.
const MAX_CLUSTER_BASES: usize = 8;
/// Fixed per-line scan overhead on top of the per-cell budget, so short
/// panes can still consume ordinary escape prefixes before the bounded
/// prefix limit stops the scan.
const SCAN_OVERHEAD_BYTES: usize = 256;
/// ZWJ (U+200D): joins the next base into one cluster.
const ZWJ: char = '\u{200D}';
/// Variation selector-16 (U+FE0F): requests emoji presentation (two cells).
const VS16: char = '\u{FE0F}';
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
        if cluster.last_was_zwj && cluster.can_join(c) && !cluster.composed {
            // ZWJ + following base stays one cluster (bounded join chain). A
            // composed Hangul syllable never absorbs that join: the pinned
            // oracle breaks there because conjoining jamo are not
            // Extended_Pictographic (executed `1100 1161 200D 1F680` = 4).
            cluster.join(c, w);
            continue;
        }
        if cluster.can_extend_hangul(c) {
            // Decomposed Hangul stays one syllable cluster (L+V, LV+T).
            cluster.join_hangul(c, w);
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
        if !cluster.is_empty() && cluster.can_extend_hangul(c) {
            // Same conjoining-Hangul rule as `fit`, so measurement and
            // slicing cannot disagree about a syllable's cells.
            cluster.join_hangul(c, usize::from(char_width(c)));
            continue;
        }
        if !cluster.is_empty()
            && ((cluster.last_was_zwj && cluster.can_join(c) && !cluster.composed)
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
    /// A variation selector-16 (U+FE0F) requests emoji presentation, which the
    /// pinned renderer widens to two cells even for an ASCII base.
    vs16: bool,
    /// The base chain is a conjoining Hangul syllable (L+V or L+V+T).
    composed: bool,
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
        self.vs16 = false;
        self.composed = false;
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
        } else if c == VS16 {
            self.vs16 = true;
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

    /// Join a conjoining Hangul jamo (L+V or LV+T). The syllable renders as the
    /// widest base, never the scalar sum.
    fn join_hangul(&mut self, c: char, w: usize) {
        self.join(c, w);
        self.composed = true;
    }

    /// Whether `c` continues a conjoining Hangul syllable (UAX #29 GB6/GB7
    /// subset): a medial vowel after a leading consonant, or a final consonant
    /// after the letter+vowel syllable. Marks and ZWJ never take this path.
    fn can_extend_hangul(&self, c: char) -> bool {
        if self.conts != 0 || self.zwj || self.regional != 0 {
            return false;
        }
        let Some(last) = self.text.chars().last() else {
            return false;
        };
        let (last, next) = (last as u32, c as u32);
        ((0x1100..=0x115F).contains(&last) && (0x1160..=0x11A7).contains(&next))
            || ((0x1160..=0x11A7).contains(&last) && (0x11A8..=0x11FF).contains(&next))
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
        let mut width = if self.regional == 2 {
            // A completed flag pair is one 2-cell glyph; never the naive
            // scalar sum of its halves (4 when the width table classifies
            // regional indicators as wide).
            FLAG_PAIR_CELLS
        } else if (self.zwj || self.composed) && self.bases >= 2 {
            // A joined emoji sequence or a conjoining Hangul syllable occupies
            // the width of its widest base, not the scalar sum.
            self.max
        } else {
            self.sum
        };
        if self.vs16 {
            // Emoji presentation is two cells regardless of the base: the
            // pinned renderer measures `a`+U+FE0F and `1`+U+FE0F as 2.
            width = width.max(2);
        }
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
        self.vs16 = false;
        self.composed = false;
    }
}

/// Display cells occupied by `c`: 0 for combining/zero-width marks, 2 for
/// wide (CJK, fullwidth, emoji) scalars, 1 otherwise.
///
/// The per-scalar table is the executed pinned-renderer `unicode` width table
/// (`WIDE_RANGES` / `ZERO_WIDTH_RANGES`), swept over the whole codespace
/// `0x0000..=0x10FFFF`. The cluster *segmentation* around it is still a bounded
/// heuristic subset of UAX #29 (see [`Cluster`]), so a grapheme rule this module
/// does not model (a ZWJ chain longer than the cluster byte/base caps, a ZWJ
/// after a non-emoji base, or a script-specific conjoining rule) can still
/// differ from the renderer. Upgrade path: the `unicode-segmentation` crate
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

/// Combining marks, variation selectors, format characters and the
/// default-ignorable supplementary plane (width 0).
///
/// Derived by an executed single-scalar sweep of the pinned renderer,
/// OpenTUI 0.6.8 `measureEditText(text, { widthMethod: "unicode", tabWidth: 2 })`,
/// over the whole codespace `0x0000..=0x10FFFF` — 1_112_064 measured scalars,
/// lone surrogates excluded (`cell-width-implementation/supplementary-repair/
/// probe-supplementary.json`, which extends the earlier `0x0000..=0x3FFFD`
/// sweep in `probe-bmp.json`). Every scalar listed measures 0 there, so this is
/// the renderer's mark/format table rather than a hand-picked fixture list.
const ZERO_WIDTH_RANGES: &[(u32, u32)] = &[
    (0x0300, 0x036F),
    (0x0483, 0x0489),
    (0x0591, 0x05BD),
    (0x05BF, 0x05BF),
    (0x05C1, 0x05C2),
    (0x05C4, 0x05C5),
    (0x05C7, 0x05C7),
    (0x0610, 0x061A),
    (0x061C, 0x061C),
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
    (0x07FD, 0x07FD),
    (0x0816, 0x0819),
    (0x081B, 0x0823),
    (0x0825, 0x0827),
    (0x0829, 0x082D),
    (0x0859, 0x085B),
    (0x0897, 0x089F),
    (0x08CA, 0x08E1),
    (0x08E3, 0x0903),
    (0x093A, 0x093C),
    (0x093E, 0x094F),
    (0x0951, 0x0957),
    (0x0962, 0x0963),
    (0x0981, 0x0983),
    (0x09BC, 0x09BC),
    (0x09BE, 0x09C4),
    (0x09C7, 0x09C8),
    (0x09CB, 0x09CD),
    (0x09D7, 0x09D7),
    (0x09E2, 0x09E3),
    (0x09FE, 0x09FE),
    (0x0A01, 0x0A03),
    (0x0A3C, 0x0A3C),
    (0x0A3E, 0x0A42),
    (0x0A47, 0x0A48),
    (0x0A4B, 0x0A4D),
    (0x0A51, 0x0A51),
    (0x0A70, 0x0A71),
    (0x0A75, 0x0A75),
    (0x0A81, 0x0A83),
    (0x0ABC, 0x0ABC),
    (0x0ABE, 0x0AC5),
    (0x0AC7, 0x0AC9),
    (0x0ACB, 0x0ACD),
    (0x0AE2, 0x0AE3),
    (0x0AFA, 0x0AFF),
    (0x0B01, 0x0B03),
    (0x0B3C, 0x0B3C),
    (0x0B3E, 0x0B44),
    (0x0B47, 0x0B48),
    (0x0B4B, 0x0B4D),
    (0x0B55, 0x0B57),
    (0x0B62, 0x0B63),
    (0x0B82, 0x0B82),
    (0x0BBE, 0x0BC2),
    (0x0BC6, 0x0BC8),
    (0x0BCA, 0x0BCD),
    (0x0BD7, 0x0BD7),
    (0x0C00, 0x0C04),
    (0x0C3C, 0x0C3C),
    (0x0C3E, 0x0C44),
    (0x0C46, 0x0C48),
    (0x0C4A, 0x0C4D),
    (0x0C55, 0x0C56),
    (0x0C62, 0x0C63),
    (0x0C81, 0x0C83),
    (0x0CBC, 0x0CBC),
    (0x0CBE, 0x0CC4),
    (0x0CC6, 0x0CC8),
    (0x0CCA, 0x0CCD),
    (0x0CD5, 0x0CD6),
    (0x0CE2, 0x0CE3),
    (0x0CF3, 0x0CF3),
    (0x0D00, 0x0D03),
    (0x0D3B, 0x0D3C),
    (0x0D3E, 0x0D44),
    (0x0D46, 0x0D48),
    (0x0D4A, 0x0D4D),
    (0x0D57, 0x0D57),
    (0x0D62, 0x0D63),
    (0x0D81, 0x0D83),
    (0x0DCA, 0x0DCA),
    (0x0DCF, 0x0DD4),
    (0x0DD6, 0x0DD6),
    (0x0DD8, 0x0DDF),
    (0x0DF2, 0x0DF3),
    (0x0E31, 0x0E31),
    (0x0E34, 0x0E3A),
    (0x0E47, 0x0E4E),
    (0x0EB1, 0x0EB1),
    (0x0EB4, 0x0EBC),
    (0x0EC8, 0x0ECE),
    (0x0F18, 0x0F19),
    (0x0F35, 0x0F35),
    (0x0F37, 0x0F37),
    (0x0F39, 0x0F39),
    (0x0F3E, 0x0F3F),
    (0x0F71, 0x0F84),
    (0x0F86, 0x0F87),
    (0x0F8D, 0x0F97),
    (0x0F99, 0x0FBC),
    (0x0FC6, 0x0FC6),
    (0x102B, 0x103E),
    (0x1056, 0x1059),
    (0x105E, 0x1060),
    (0x1062, 0x1064),
    (0x1067, 0x106D),
    (0x1071, 0x1074),
    (0x1082, 0x108D),
    (0x108F, 0x108F),
    (0x109A, 0x109D),
    (0x135D, 0x135F),
    (0x1712, 0x1715),
    (0x1732, 0x1734),
    (0x1752, 0x1753),
    (0x1772, 0x1773),
    (0x17B4, 0x17D3),
    (0x17DD, 0x17DD),
    (0x180B, 0x180F),
    (0x1885, 0x1886),
    (0x18A9, 0x18A9),
    (0x1920, 0x192B),
    (0x1930, 0x193B),
    (0x1A17, 0x1A1B),
    (0x1A55, 0x1A5E),
    (0x1A60, 0x1A7C),
    (0x1A7F, 0x1A7F),
    (0x1AB0, 0x1ADD),
    (0x1AE0, 0x1AEB),
    (0x1B00, 0x1B04),
    (0x1B34, 0x1B44),
    (0x1B6B, 0x1B73),
    (0x1B80, 0x1B82),
    (0x1BA1, 0x1BAD),
    (0x1BE6, 0x1BF3),
    (0x1C24, 0x1C37),
    (0x1CD0, 0x1CD2),
    (0x1CD4, 0x1CE8),
    (0x1CED, 0x1CED),
    (0x1CF4, 0x1CF4),
    (0x1CF7, 0x1CF9),
    (0x1DC0, 0x1DFF),
    (0x200B, 0x200F),
    (0x202A, 0x202E),
    (0x2060, 0x206F),
    (0x20D0, 0x20F0),
    (0x2CEF, 0x2CF1),
    (0x2D7F, 0x2D7F),
    (0x2DE0, 0x2DFF),
    (0x302A, 0x302F),
    (0x3099, 0x309A),
    (0x3164, 0x3164),
    (0xA66F, 0xA672),
    (0xA674, 0xA67D),
    (0xA69E, 0xA69F),
    (0xA6F0, 0xA6F1),
    (0xA802, 0xA802),
    (0xA806, 0xA806),
    (0xA80B, 0xA80B),
    (0xA823, 0xA827),
    (0xA82C, 0xA82C),
    (0xA880, 0xA881),
    (0xA8B4, 0xA8C5),
    (0xA8E0, 0xA8F1),
    (0xA8FF, 0xA8FF),
    (0xA926, 0xA92D),
    (0xA947, 0xA953),
    (0xA980, 0xA983),
    (0xA9B3, 0xA9C0),
    (0xA9E5, 0xA9E5),
    (0xAA29, 0xAA36),
    (0xAA43, 0xAA43),
    (0xAA4C, 0xAA4D),
    (0xAA7B, 0xAA7D),
    (0xAAB0, 0xAAB0),
    (0xAAB2, 0xAAB4),
    (0xAAB7, 0xAAB8),
    (0xAABE, 0xAABF),
    (0xAAC1, 0xAAC1),
    (0xAAEB, 0xAAEF),
    (0xAAF5, 0xAAF6),
    (0xABE3, 0xABEA),
    (0xABEC, 0xABED),
    (0xFB1E, 0xFB1E),
    (0xFE00, 0xFE0F),
    (0xFE20, 0xFE2F),
    (0xFEFF, 0xFEFF),
    (0xFFA0, 0xFFA0),
    (0xFFF0, 0xFFF8),
    (0x101FD, 0x101FD),
    (0x102E0, 0x102E0),
    (0x10376, 0x1037A),
    (0x10A01, 0x10A03),
    (0x10A05, 0x10A06),
    (0x10A0C, 0x10A0F),
    (0x10A38, 0x10A3A),
    (0x10A3F, 0x10A3F),
    (0x10AE5, 0x10AE6),
    (0x10D24, 0x10D27),
    (0x10D69, 0x10D6D),
    (0x10EAB, 0x10EAC),
    (0x10EFA, 0x10EFF),
    (0x10F46, 0x10F50),
    (0x10F82, 0x10F85),
    (0x11000, 0x11002),
    (0x11038, 0x11046),
    (0x11070, 0x11070),
    (0x11073, 0x11074),
    (0x1107F, 0x11082),
    (0x110B0, 0x110BA),
    (0x110C2, 0x110C2),
    (0x11100, 0x11102),
    (0x11127, 0x11134),
    (0x11145, 0x11146),
    (0x11173, 0x11173),
    (0x11180, 0x11182),
    (0x111B3, 0x111C0),
    (0x111C9, 0x111CC),
    (0x111CE, 0x111CF),
    (0x1122C, 0x11237),
    (0x1123E, 0x1123E),
    (0x11241, 0x11241),
    (0x112DF, 0x112EA),
    (0x11300, 0x11303),
    (0x1133B, 0x1133C),
    (0x1133E, 0x11344),
    (0x11347, 0x11348),
    (0x1134B, 0x1134D),
    (0x11357, 0x11357),
    (0x11362, 0x11363),
    (0x11366, 0x1136C),
    (0x11370, 0x11374),
    (0x113B8, 0x113C0),
    (0x113C2, 0x113C2),
    (0x113C5, 0x113C5),
    (0x113C7, 0x113CA),
    (0x113CC, 0x113D0),
    (0x113D2, 0x113D2),
    (0x113E1, 0x113E2),
    (0x11435, 0x11446),
    (0x1145E, 0x1145E),
    (0x114B0, 0x114C3),
    (0x115AF, 0x115B5),
    (0x115B8, 0x115C0),
    (0x115DC, 0x115DD),
    (0x11630, 0x11640),
    (0x116AB, 0x116B7),
    (0x1171D, 0x1172B),
    (0x1182C, 0x1183A),
    (0x11930, 0x11935),
    (0x11937, 0x11938),
    (0x1193B, 0x1193E),
    (0x11940, 0x11940),
    (0x11942, 0x11943),
    (0x119D1, 0x119D7),
    (0x119DA, 0x119E0),
    (0x119E4, 0x119E4),
    (0x11A01, 0x11A0A),
    (0x11A33, 0x11A39),
    (0x11A3B, 0x11A3E),
    (0x11A47, 0x11A47),
    (0x11A51, 0x11A5B),
    (0x11A8A, 0x11A99),
    (0x11B60, 0x11B67),
    (0x11C2F, 0x11C36),
    (0x11C38, 0x11C3F),
    (0x11C92, 0x11CA7),
    (0x11CA9, 0x11CB6),
    (0x11D31, 0x11D36),
    (0x11D3A, 0x11D3A),
    (0x11D3C, 0x11D3D),
    (0x11D3F, 0x11D45),
    (0x11D47, 0x11D47),
    (0x11D8A, 0x11D8E),
    (0x11D90, 0x11D91),
    (0x11D93, 0x11D97),
    (0x11EF3, 0x11EF6),
    (0x11F00, 0x11F01),
    (0x11F03, 0x11F03),
    (0x11F34, 0x11F3A),
    (0x11F3E, 0x11F42),
    (0x11F5A, 0x11F5A),
    (0x13440, 0x13440),
    (0x13447, 0x13455),
    (0x1611E, 0x1612F),
    (0x16AF0, 0x16AF4),
    (0x16B30, 0x16B36),
    (0x16F4F, 0x16F4F),
    (0x16F51, 0x16F87),
    (0x16F8F, 0x16F92),
    (0x16FE4, 0x16FE4),
    (0x16FF0, 0x16FF1),
    (0x1BC9D, 0x1BC9E),
    (0x1BCA0, 0x1BCA3),
    (0x1CF00, 0x1CF2D),
    (0x1CF30, 0x1CF46),
    (0x1D165, 0x1D169),
    (0x1D16D, 0x1D182),
    (0x1D185, 0x1D18B),
    (0x1D1AA, 0x1D1AD),
    (0x1D242, 0x1D244),
    (0x1DA00, 0x1DA36),
    (0x1DA3B, 0x1DA6C),
    (0x1DA75, 0x1DA75),
    (0x1DA84, 0x1DA84),
    (0x1DA9B, 0x1DA9F),
    (0x1DAA1, 0x1DAAF),
    (0x1E000, 0x1E006),
    (0x1E008, 0x1E018),
    (0x1E01B, 0x1E021),
    (0x1E023, 0x1E024),
    (0x1E026, 0x1E02A),
    (0x1E08F, 0x1E08F),
    (0x1E130, 0x1E136),
    (0x1E2AE, 0x1E2AE),
    (0x1E2EC, 0x1E2EF),
    (0x1E4EC, 0x1E4EF),
    (0x1E5EE, 0x1E5EF),
    (0x1E6E3, 0x1E6E3),
    (0x1E6E6, 0x1E6E6),
    (0x1E6EE, 0x1E6EF),
    (0x1E6F5, 0x1E6F5),
    (0x1E8D0, 0x1E8D6),
    (0x1E944, 0x1E94A),
    // Default-ignorable plane 14: tag scalars (U+E0020..U+E007F) that compose
    // RGI subdivision flags, and variation selectors 17..256 used by
    // ideographic variation sequences. Executed width 0.
    (0xE0000, 0xE0FFF),
];

/// East Asian Wide / Fullwidth, emoji-presentation and other 2-cell scalars.
///
/// Same executed full-codespace sweep of the pinned renderer (see
/// `ZERO_WIDTH_RANGES`); nothing above `0x3FFFD` measures 2. C0, C1 and DEL
/// scalars are excluded on purpose: their width in the renderer comes from
/// control/caret notation, which the prompt surface (`native_prompt_layout`)
/// owns, not this module.
const WIDE_RANGES: &[(u32, u32)] = &[
    (0x1100, 0x115F),
    (0x203C, 0x203C),
    (0x2049, 0x2049),
    (0x231A, 0x231B),
    (0x2329, 0x232A),
    (0x23E9, 0x23EC),
    (0x23F0, 0x23F0),
    (0x23F3, 0x23F3),
    (0x25FD, 0x25FE),
    (0x2614, 0x2615),
    (0x2622, 0x2623),
    (0x2630, 0x2637),
    (0x2648, 0x2653),
    (0x267F, 0x267F),
    (0x268A, 0x268F),
    (0x2693, 0x2693),
    (0x269B, 0x269B),
    (0x26A1, 0x26A1),
    (0x26AA, 0x26AB),
    (0x26BD, 0x26BE),
    (0x26C4, 0x26C5),
    (0x26CE, 0x26CE),
    (0x26D1, 0x26D1),
    (0x26D4, 0x26D4),
    (0x26EA, 0x26EA),
    (0x26F2, 0x26F3),
    (0x26F5, 0x26F5),
    (0x26FA, 0x26FA),
    (0x26FD, 0x26FD),
    (0x2705, 0x2705),
    (0x270A, 0x270B),
    (0x2728, 0x2728),
    (0x274C, 0x274C),
    (0x274E, 0x274E),
    (0x2753, 0x2755),
    (0x2757, 0x2757),
    (0x2760, 0x2767),
    (0x2795, 0x2797),
    (0x27B0, 0x27B0),
    (0x27BF, 0x27BF),
    (0x2B1B, 0x2B1C),
    (0x2B50, 0x2B50),
    (0x2B55, 0x2B55),
    (0x2E80, 0x2E99),
    (0x2E9B, 0x2EF3),
    (0x2F00, 0x2FD5),
    (0x2FF0, 0x3029),
    (0x3030, 0x303E),
    (0x3041, 0x3096),
    (0x309B, 0x30FF),
    (0x3105, 0x312F),
    (0x3131, 0x3163),
    (0x3165, 0x318E),
    (0x3190, 0x31E5),
    (0x31EF, 0x321E),
    (0x3220, 0x3247),
    (0x3250, 0xA48C),
    (0xA490, 0xA4C6),
    (0xA960, 0xA97C),
    (0xAC00, 0xD7A3),
    (0xF900, 0xFAFF),
    (0xFE10, 0xFE19),
    (0xFE30, 0xFE52),
    (0xFE54, 0xFE66),
    (0xFE68, 0xFE6B),
    (0xFF01, 0xFF60),
    (0xFFE0, 0xFFE6),
    (0x16FE0, 0x16FE3),
    (0x16FF2, 0x16FF6),
    (0x17000, 0x18CD5),
    (0x18CFF, 0x18D1E),
    (0x18D80, 0x18DF2),
    (0x1AFF0, 0x1AFF3),
    (0x1AFF5, 0x1AFFB),
    (0x1AFFD, 0x1AFFE),
    (0x1B000, 0x1B122),
    (0x1B132, 0x1B132),
    (0x1B150, 0x1B152),
    (0x1B155, 0x1B155),
    (0x1B164, 0x1B167),
    (0x1B170, 0x1B2FB),
    (0x1D300, 0x1D356),
    (0x1D360, 0x1D376),
    (0x1F000, 0x1F02B),
    (0x1F030, 0x1F093),
    (0x1F0A0, 0x1F0AE),
    (0x1F0B1, 0x1F0BF),
    (0x1F0C1, 0x1F0CF),
    (0x1F0D1, 0x1F0F5),
    (0x1F18E, 0x1F18E),
    (0x1F191, 0x1F19A),
    (0x1F200, 0x1F202),
    (0x1F210, 0x1F23B),
    (0x1F240, 0x1F248),
    (0x1F250, 0x1F251),
    (0x1F260, 0x1F265),
    (0x1F300, 0x1F320),
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
    (0x1F4FF, 0x1F6C5),
    (0x1F6CC, 0x1F6CC),
    (0x1F6D0, 0x1F6D2),
    (0x1F6D5, 0x1F6D8),
    (0x1F6DC, 0x1F6DF),
    (0x1F6EB, 0x1F6EC),
    (0x1F6F4, 0x1F6FC),
    (0x1F700, 0x1F773),
    (0x1F780, 0x1F7D8),
    (0x1F7E0, 0x1F7EB),
    (0x1F7F0, 0x1F7F0),
    (0x1F800, 0x1F80B),
    (0x1F810, 0x1F847),
    (0x1F850, 0x1F859),
    (0x1F860, 0x1F887),
    (0x1F890, 0x1F8AD),
    (0x1F8B0, 0x1F8B1),
    (0x1F90C, 0x1F93A),
    (0x1F93C, 0x1F945),
    (0x1F947, 0x1FA53),
    (0x1FA60, 0x1FA6D),
    (0x1FA70, 0x1FA7C),
    (0x1FA80, 0x1FA8A),
    (0x1FA8E, 0x1FAC6),
    (0x1FAC8, 0x1FAC8),
    (0x1FACD, 0x1FADC),
    (0x1FADF, 0x1FAEA),
    (0x1FAEF, 0x1FAF8),
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
// `native_shell_frame`. This module stays self-contained (no dependency on the
// rest of the view layer) so it remains usable and testable in isolation.
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

/// Resolved right-pane geometry for one live session frame — the single
/// source the facade's `presentation()` and the paint seam must consume
/// (`component/session-frame.tsx:262-391` at the pinned revision).
///
/// Upstream model:
/// - visibility (`session-frame.tsx:109-115`): a parent session never shows
///   the sidebar; an explicit open always shows it; otherwise the `auto`
///   default shows it only while `wide` — `width - verticalTabsWidth > 120`
///   (`:106`, strict at 120; [`session_wide`]).
/// - precedence (`:115-120`, [`right_pane`]): an active panel (oc2's dock)
///   wins, then the sidebar, then a terminal.
/// - docked geometry (`:326-336`): the right pane is a non-shrinking flex
///   sibling (`flexShrink={0}`, width [`SESSION_SIDEBAR_WIDTH`] for the
///   sidebar) while `#session-pane` shrinks with it (`flexGrow={1}
///   flexBasis={0} minWidth={0}`, `:269-274`) — the transcript can never
///   paint under a docked pane.
/// - narrow geometry (`:383-391`): below the wide threshold the sidebar is an
///   ABSOLUTE drawer anchored right over the content with a dim backdrop
///   (`RGBA.fromInts(0, 0, 0, 70)`) — content intentionally paints under it.
///
/// `docked` distinguishes the two regimes: `true` means the returned
/// `main_width` already excludes the pane, so no main-column cell can reach
/// the pane's columns; `false` means the caller owns the drawer contract
/// (upstream's backdrop dim, or an explicit shrink decision).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionPaneLayout {
    /// First main-column cell (the vertical-tab rail offset).
    pub main_offset: u16,
    /// Main-column width in cells.
    pub main_width: u16,
    /// Right-pane start cell, or `None` when no pane is visible.
    pub sidebar_offset: Option<u16>,
    /// Right-pane width in cells (`0` when no pane is visible).
    pub sidebar_width: u16,
    /// `true` when the pane is a docked flex sibling (the main column shrunk
    /// by [`right_pane_width`]); `false` for the narrow absolute drawer.
    pub docked: bool,
}

/// Resolve the live right-pane geometry from the upstream session-frame
/// model. `pane_resize` is the user-resized terminal/panel width (the
/// sidebar always takes [`SESSION_SIDEBAR_WIDTH`]). `session_route` carries
/// oc2's routing fact: the sidebar lives inside the session frame
/// (`session-frame.tsx` renders it), so a non-session route (home) never
/// reserves the pane — the reviewed LAY-08 `sidebar_visible` gate.
#[must_use]
pub fn session_pane_layout(
    cols: u16,
    tab_offset: u16,
    rows: u16,
    session_route: bool,
    parent_session: bool,
    sidebar_open: bool,
    sidebar_hidden: bool,
    dock_open: bool,
    pane_resize: u16,
) -> SessionPaneLayout {
    // Visibility (`session-frame.tsx:109-115`): the session frame owns the
    // pane, parent sessions never show the sidebar; an explicit open wins
    // over the auto gate; `auto` shows it only on a wide viewport. oc2
    // additionally gates on `rows >= 2` (LAY-08's reviewed `sidebar_visible`).
    let wide = session_wide(cols, tab_offset);
    let sidebar_visible = session_route
        && !parent_session
        && rows >= 2
        && (sidebar_open || (!sidebar_hidden && wide));
    let pane = right_pane(dock_open, sidebar_open, sidebar_visible, false);
    let pane_width = right_pane_width(pane, pane_resize);
    // A terminal/panel pane is always a docked sibling
    // (`session-frame.tsx:326-336`); the sidebar docks only while wide and
    // otherwise renders as the `:383-391` drawer.
    let docked = match pane {
        RightPane::Sidebar => wide,
        RightPane::None => false,
        RightPane::Panel | RightPane::Terminal => true,
    };
    // The pane's painted width regardless of regime; `docked` tells the
    // caller whether `main_width` already excludes it (docked) or the pane
    // overlays those columns (drawer).
    let sidebar_width = pane_width;
    let main_width = cols
        .saturating_sub(tab_offset)
        .saturating_sub(if docked { pane_width } else { 0 });
    let sidebar_offset = (pane_width > 0).then_some(cols.saturating_sub(pane_width));
    SessionPaneLayout {
        main_offset: tab_offset,
        main_width,
        sidebar_offset,
        sidebar_width,
        docked,
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
// it stays usable without the rest of the view layer.
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

    // RIGHT-PANEL-OVERLAP: the single-source right-pane geometry authority.
    // Upstream `component/session-frame.tsx:262-391` at pin
    // `450e7b92412c780a7b5a75226bb8d49aaca24e22`.
    #[test]
    fn session_pane_layout_docks_and_shrinks_main_at_140x40() {
        // Wide viewport (140 > 120), sidebar auto-visible: the pane is a
        // docked flex sibling and the main column shrinks to its left edge
        // (`session-frame.tsx:269-274,326-336`).
        let layout = session_pane_layout(140, 0, 40, true, false, false, false, false, 70);
        assert_eq!(
            layout,
            SessionPaneLayout {
                main_offset: 0,
                main_width: 98,
                sidebar_offset: Some(98),
                sidebar_width: SESSION_SIDEBAR_WIDTH,
                docked: true,
            }
        );
    }

    #[test]
    fn session_pane_layout_overlay_prescribes_the_upstream_drawer() {
        // Vertical tabs (42-cell rail) keep the effective width at/below the
        // dock threshold; an explicitly opened sidebar renders as the
        // `:383-391` absolute drawer anchored right: the main column keeps
        // `cols - tab_offset` and the pane overlays `cols - 42` with the dim
        // backdrop. `docked == false` tells the caller the pane overlays.
        let layout = session_pane_layout(140, 42, 40, true, false, true, false, false, 70);
        assert_eq!(
            layout,
            SessionPaneLayout {
                main_offset: 42,
                main_width: 98,
                sidebar_offset: Some(98),
                sidebar_width: SESSION_SIDEBAR_WIDTH,
                docked: false,
            }
        );
        // Narrow terminals behave the same when the sidebar is opened
        // explicitly (`sidebarVisible`, `session-frame.tsx:109-115`).
        let narrow = session_pane_layout(100, 0, 30, true, false, true, false, false, 50);
        assert_eq!(narrow.main_width, 100);
        assert_eq!(narrow.sidebar_offset, Some(58));
        assert!(!narrow.docked);
    }

    #[test]
    fn session_pane_layout_panel_and_terminal_panes_always_dock() {
        // `rightPane()` precedence (`session-frame.tsx:115-120`): an active
        // panel wins over an open sidebar, and both terminal/panel panes are
        // docked siblings at the resized width (`:329-336`).
        let panel = session_pane_layout(140, 0, 40, true, false, true, false, true, 60);
        assert_eq!(panel.sidebar_width, 60);
        assert_eq!(panel.sidebar_offset, Some(80));
        assert_eq!(panel.main_width, 80);
        assert!(panel.docked);
        // Without a dock the auto sidebar docks at the fixed 42-cell width.
        let sidebar = session_pane_layout(140, 0, 40, true, false, false, false, false, 60);
        assert_eq!(sidebar.sidebar_width, SESSION_SIDEBAR_WIDTH);
        assert!(sidebar.docked);
    }

    #[test]
    fn session_pane_layout_parent_and_hidden_gates_suppress_the_sidebar() {
        // A parent session never shows the sidebar
        // (`session-frame.tsx:110`).
        let parent = session_pane_layout(140, 0, 40, true, true, true, false, false, 70);
        assert_eq!(parent.sidebar_offset, None);
        assert_eq!(parent.sidebar_width, 0);
        assert_eq!(parent.main_width, 140);
        assert!(!parent.docked);
        // `session.sidebar = "hide"` suppresses the auto-open even when wide;
        // only an explicit open shows it (`session-frame.tsx:112-114`).
        let hidden = session_pane_layout(140, 0, 40, true, false, false, true, false, 70);
        assert_eq!(hidden.sidebar_offset, None);
        assert_eq!(hidden.main_width, 140);
        // Short viewports keep oc2's reviewed rows >= 2 gate.
        let short = session_pane_layout(140, 0, 1, true, false, true, false, false, 70);
        assert_eq!(short.sidebar_offset, None);
    }

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

    // -----------------------------------------------------------------------
    // UI-PARITY-CELL-WIDTH-20261113: shared Unicode cell-width parity.
    //
    // Every expectation below is an **executed** value of the pinned renderer,
    // never a reading of our own model:
    //
    //   measureEditText(text, { widthMethod: "unicode", tabWidth: 2 })
    //
    // from the pinned OpenTUI 0.6.8 install
    // `artifacts/upstream-opentui068/node_modules/.bun/@opentui+core@0.6.8+
    //  eff4d38d6a4a2cd9/node_modules/@opentui/core` (package.json
    // `68b72bb4a373514f272ae2c060f8962e1cd49ffb86176b78660a480b5b3f6db7`,
    // edit-buffer.d.ts `1b2091483e11ee99c1f353e91bf151ce66154ee96ca3ba87feb5
    // 40efc4e5f514`, chunk-node-jhq62x0c.js `2ec40eb00d62da309874e8114e0fcf
    // f451b93f920adf1baf4f4979838daf6ec0`). This is the exact call the pinned
    // application makes (`packages/tui/src/component/prompt/index.tsx:217`,
    // `:1097-1404`) with `renderer.widthMethod`, whose 0.6.8 default is
    // `"unicode"` (`chunk-node-jhq62x0c.js:8587`, `options.widthMethod ??
    // "unicode"`), pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`.
    //
    // Row values are frozen from the executed oracle
    // (`artifacts/ui-parity-autonomous-20261113/cell-width/oracle.json`,
    // sha256 472a81aa…, 94 assertions, rc=0) and the single-scalar sweep of the
    // same API recorded in
    // `cell-width-implementation/probe-bmp.json` (260_094 measured scalars).
    //
    // Deliberately NOT asserted here: C0/C1 control *caret notation* (`\f` ->
    // `^L`, C1 -> 2-cell caret) is the paste/prompt owner's behaviour
    // (`native_prompt_layout`), and a tab is expanded to two cells by that
    // prompt surface (`native_prompt_layout.rs:16-19`) before this model runs,
    // so tab rows are asserted through that documented composition rather than
    // claimed for this module.
    // -----------------------------------------------------------------------

    /// Unpadded display cells of `text` under the production clustering model.
    fn model_cells(text: &str) -> usize {
        visit_unpadded_cells(text, 4096, |_, _| {})
    }

    #[test]
    fn ui_parity_cell_width_oracle_unicode_method() {
        // (fixture id, text, executed `unicode` cells)
        const ROWS: &[(&str, &str, usize)] = &[
            // A: emoji-presentation scalars (wide even without VS16).
            ("heart-bare", "\u{2764}", 2),
            ("ep-check-mark", "\u{2705}", 2),
            ("ep-sparkles", "\u{2728}", 2),
            ("ep-star", "\u{2B50}", 2),
            ("ep-no-entry", "\u{26D4}", 2),
            ("ep-snowman", "\u{26C4}", 2),
            ("ep-umbrella", "\u{2614}", 2),
            ("ep-alarm-clock", "\u{23F0}", 2),
            ("ep-hourglass", "\u{231B}", 2),
            ("ep-watch", "\u{231A}", 2),
            // B: VS16 presents an otherwise text-default base as emoji.
            ("vs16-heart", "\u{2764}\u{FE0F}", 2),
            ("vs16-smiley", "\u{263A}\u{FE0F}", 2),
            ("vs16-copyright", "\u{00A9}\u{FE0F}", 2),
            ("vs16-trade-mark", "\u{2122}\u{FE0F}", 2),
            ("vs16-registered", "\u{00AE}\u{FE0F}", 2),
            ("vs16-warning", "\u{26A0}\u{FE0F}", 2),
            ("vs16-airplane", "\u{2708}\u{FE0F}", 2),
            ("vs16-info", "\u{2139}\u{FE0F}", 2),
            ("vs16-arrow", "\u{2194}\u{FE0F}", 2),
            ("vs16-ascii-a", "a\u{FE0F}", 2),
            ("keycap-1", "1\u{FE0F}\u{20E3}", 2),
            ("keycap-12", "1\u{FE0F}\u{20E3}2\u{FE0F}\u{20E3}", 4),
            // C: Indic dependent signs are zero-width editor marks.
            ("devanagari-ki", "\u{0915}\u{093F}", 1),
            ("bengali-ki", "\u{0995}\u{09BF}", 1),
            // D: decomposed Hangul conjoining jamo is one syllable cluster.
            ("hangul-jamo-decomposed", "\u{1100}\u{1161}", 2),
            // E: a five-member ZWJ family is one 2-cell cluster.
            (
                "zwj-family-5",
                "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}\u{200D}\u{1F466}",
                2,
            ),
            // Matching controls: must not regress.
            ("cjk-1", "\u{4E2D}", 2),
            ("cjk-2", "\u{4E2D}\u{6587}", 4),
            ("cjk-hangul", "\u{AC00}", 2),
            ("cjk-mention", "@\u{52A9}\u{624B}", 5),
            ("combining-e-acute", "e\u{0301}", 1),
            ("combining-stacked", "o\u{0308}\u{0323}", 1),
            ("precomposed-e-acute", "\u{00E9}", 1),
            ("tamil-ni", "\u{0BA8}\u{0BBF}", 1),
            ("thai-ko-i", "\u{0E01}\u{0E34}", 1),
            (
                "zwj-family-3",
                "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
                2,
            ),
            (
                "zwj-family-4",
                "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}",
                2,
            ),
            ("zwj-flag-rainbow", "\u{1F3F3}\u{FE0F}\u{200D}\u{1F308}", 2),
            ("skin-tone-thumbs", "\u{1F44D}\u{1F3FD}", 2),
            (
                "skin-tone-zwj-astronaut",
                "\u{1F9D1}\u{1F3FD}\u{200D}\u{1F680}",
                2,
            ),
            ("flag-in", "\u{1F1EE}\u{1F1F3}", 2),
            ("keycap-bare-1", "1\u{20E3}", 1),
            ("tp-ballot-check", "\u{2611}", 1),
            ("tp-play", "\u{25B6}", 1),
            ("tp-heavy-check", "\u{2714}", 1),
            ("tp-sun", "\u{2600}", 1),
            ("amb-star", "\u{2605}", 1),
            ("amb-omega", "\u{03A9}", 1),
            ("amb-arrow", "\u{2192}", 1),
            ("amb-plus-minus", "\u{00B1}", 1),
            ("amb-section", "\u{00A7}", 1),
            ("amb-circled-1", "\u{2460}", 1),
            ("amb-box-drawing", "\u{2500}", 1),
        ];
        for (id, text, expected) in ROWS {
            assert_eq!(
                model_cells(text),
                *expected,
                "cell width of fixture {id} ({text:?}) must match the pinned \
                 unicode method"
            );
        }

        // TAB-08 / SIDE-13: the prompt surface expands `\t` to two cells
        // before this model measures (`native_prompt_layout.rs:16-19`).
        const TABS: &[(&str, &str, usize)] = &[
            ("tab-default", "a\tb", 4),
            ("tab-cjk-zwj-tamil", "\u{4E2D}\t\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{0BA8}\u{0BBF}", 7),
        ];
        for (id, text, expected) in TABS {
            assert_eq!(
                model_cells(&text.replace('\t', "  ")),
                *expected,
                "prompt-composed tab fixture {id}"
            );
        }

        // The general per-scalar table, executed from the same pinned API, not
        // just the repaired fixtures: symbols and emoji that must widen, marks
        // that must not, and text scalars that must stay one cell.
        const SINGLE: &[(char, usize)] = &[
            ('\u{203C}', 2),
            ('\u{2049}', 2),
            ('\u{2329}', 2),
            ('\u{232A}', 2),
            ('\u{23E9}', 2),
            ('\u{25FD}', 2),
            ('\u{267F}', 2),
            ('\u{2B1B}', 2),
            ('\u{2B55}', 2),
            ('\u{4DC0}', 2),
            ('\u{1F004}', 2),
            ('\u{1F0CF}', 2),
            ('\u{1F6D0}', 2),
            ('\u{1F7E0}', 2),
            ('\u{1F90C}', 2),
            ('\u{1F9FF}', 2),
            ('\u{1FA70}', 2),
            ('\u{1FAF8}', 2),
            ('\u{3164}', 0),
            ('\u{302A}', 0),
            ('\u{0903}', 0),
            ('\u{093F}', 0),
            ('\u{09BF}', 0),
            ('\u{0ABF}', 0),
            ('\u{20E3}', 0),
            ('\u{FE0F}', 0),
            ('\u{2500}', 1),
            ('\u{2502}', 1),
            ('\u{2605}', 1),
            ('\u{303F}', 1),
            ('\u{FF00}', 1),
            ('\u{1161}', 1),
            ('\u{11A8}', 1),
            ('\u{1100}', 2),
            ('\u{2764}', 2),
            ('\u{2705}', 2),
            ('\u{2611}', 1),
            ('\u{25B6}', 1),
            ('\u{2714}', 1),
            ('\u{2600}', 1),
            ('\u{00A9}', 1),
            ('\u{2122}', 1),
            ('\u{00AE}', 1),
            ('\u{26A0}', 1),
            ('\u{2708}', 1),
            ('\u{2139}', 1),
            ('\u{2194}', 1),
        ];
        for (scalar, expected) in SINGLE {
            assert_eq!(
                model_cells(&scalar.to_string()),
                *expected,
                "single scalar U+{:04X} width",
                *scalar as u32
            );
        }

        // Hangul conjoining jamo: L+V and L+V+T are one syllable (executed 2);
        // a standalone medial or final jamo keeps its own one cell (executed 1).
        assert_eq!(model_cells("\u{1100}\u{1161}"), 2);
        assert_eq!(model_cells("\u{1100}\u{1161}\u{11A8}"), 2);
        assert_eq!(model_cells("\u{1161}"), 1);
        assert_eq!(model_cells("\u{11A8}"), 1);
        assert_eq!(model_cells("\u{1100}\u{1161}\u{11A8}\u{1100}\u{1161}"), 4);

        // Bounded overlong-ZWJ behaviour. Within the cluster caps a longer
        // valid chain is still one 2-cell cluster (executed 2 for six members);
        // beyond the caps it must split into a bounded number of bounded
        // clusters, never an unbounded glyph or scan.
        let six = "\u{1F468}\u{200D}".repeat(5) + "\u{1F468}";
        assert_eq!(model_cells(&six), 2, "six-member ZWJ chain is one cluster");
        let twenty = "\u{1F468}\u{200D}".repeat(19) + "\u{1F468}";
        let overlong = model_cells(&twenty);
        assert!(
            (2..=8).contains(&overlong),
            "overlong ZWJ chain stays bounded, got {overlong}"
        );
        let fitted = fit(&twenty, 40);
        assert_eq!(
            model_cells(&fitted),
            40,
            "the composed row stays exactly one viewport wide"
        );
        assert!(
            !fitted.trim_end_matches(' ').ends_with('\u{200D}'),
            "no dangling ZWJ in the composed row"
        );
        assert!(
            fitted.len() <= 40 * MAX_CLUSTER_BYTES + 40,
            "retained bytes stay bounded for an overlong ZWJ chain"
        );
        assert!(fitted.starts_with('\u{1F468}'));
    }

    // -----------------------------------------------------------------------
    // UI-PARITY-CELL-WIDTH-20261113 supplementary repair (review F1 + F3).
    //
    // The expectations below come from the EXTENDED executed oracle that sweeps
    // the ENTIRE Unicode codespace, not the previous `0x0000..=0x3FFFD` prefix:
    //
    //   `cell-width-implementation/supplementary-repair/probe-supplementary.json`
    //   sweep 0x0000..=0x10FFFF, 1_112_064 measured scalars (lone surrogates
    //   excluded), via the same pinned call
    //     measureEditText(text, { widthMethod: "unicode", tabWidth: 2 })
    //   against the published runtime of the selected OpenTUI pin
    //   `8d6065a3e60039d16c6ee697692dcfbc46d3c786` ("Release v0.6.8"):
    //   npm `@opentui/core@0.6.8` + `@opentui/core-darwin-arm64@0.6.8`,
    //   dylib sha256 `a2013c73d6bc9835aa59c7aa4ae6d58b0a1b6e0822a7f44ef9e21644bac72910`
    //   (source `packages/native/src/utf8.zig` `eawToWidth` at the pin).
    //   It is deliberately NOT the product bridge dylib `9e332559…`, which is a
    //   different artifact from an unpinned checkout (`c01292fd`/`4954312d`).
    //
    // OpenCode pin for the consuming surface is unchanged:
    // `450e7b92412c780a7b5a75226bb8d49aaca24e22`
    // (`packages/tui/src/component/prompt/index.tsx:217`).
    // -----------------------------------------------------------------------

    #[test]
    fn ui_parity_cell_width_supplementary_oracle_unicode_method() {
        // F1: RGI subdivision tag flags are ONE 2-cell cluster. Every tag
        // scalar (U+E0020..U+E007F) and the cancel tag (U+E007F) measure 0 in
        // the executed oracle, so the flag keeps its base width of 2.
        const FLAGS: &[(&str, &str, usize)] = &[
            (
                "subdiv-england",
                "\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}",
                2,
            ),
            (
                "subdiv-scotland",
                "\u{1F3F4}\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}\u{E007F}",
                2,
            ),
            (
                "subdiv-wales",
                "\u{1F3F4}\u{E0067}\u{E0062}\u{E0077}\u{E006C}\u{E0073}\u{E007F}",
                2,
            ),
            ("tag-flag-bare", "\u{1F3F4}", 2),
            ("lone-tag-g", "\u{E0067}", 0),
            ("two-tags", "\u{E0067}\u{E0062}", 0),
        ];
        for (id, text, expected) in FLAGS {
            assert_eq!(
                model_cells(text),
                *expected,
                "cell width of fixture {id} ({text:?}) must match the pinned \
                 full-codespace unicode oracle"
            );
        }

        // F1: ideographic variation sequences (VS17..VS256) attach at width 0,
        // so the pair keeps the base ideograph's two cells.
        const IVS: &[(&str, &str, usize)] = &[
            ("ivs-han-E0100", "\u{6F22}\u{E0100}", 2),
            ("ivs-han-E0101", "\u{6F22}\u{E0101}", 2),
            ("ivs-han-E01EF", "\u{6F22}\u{E01EF}", 2),
            ("ivs-han-noVS", "\u{6F22}", 2),
            ("ivs-two-han", "\u{6F22}\u{5B57}\u{E0100}", 4),
            ("ivs-three-han", "\u{6F22}\u{5B57}\u{6587}\u{E0100}", 6),
        ];
        for (id, text, expected) in IVS {
            assert_eq!(
                model_cells(text),
                *expected,
                "ideographic variation fixture {id} must match the pinned oracle"
            );
        }

        // F1: the whole of plane 14 measures 0 up to U+E0FFF and 1 after it —
        // the executed sweep's ONLY non-one range above U+3FFFD.
        const PLANE14: &[(char, usize)] = &[
            ('\u{E0000}', 0),
            ('\u{E0001}', 0),
            ('\u{E001F}', 0),
            ('\u{E0020}', 0),
            ('\u{E007F}', 0),
            ('\u{E0080}', 0),
            ('\u{E00FF}', 0),
            ('\u{E0100}', 0),
            ('\u{E01EF}', 0),
            ('\u{E01F0}', 0),
            ('\u{E0FFF}', 0),
            // Range boundaries immediately outside the zero range.
            ('\u{E1000}', 1),
            ('\u{E1FFF}', 1),
            ('\u{EFFFF}', 1),
            ('\u{F0000}', 1),
            ('\u{10FFFF}', 1),
        ];
        for (scalar, expected) in PLANE14 {
            assert_eq!(
                model_cells(&scalar.to_string()),
                *expected,
                "plane-14 scalar U+{:04X} width",
                *scalar as u32
            );
        }

        // Range boundaries around an existing zero range, plus the executed
        // zero/one edge scalars the pinned rules single out.
        const BOUNDARIES: &[(char, usize)] = &[
            ('\u{02FF}', 1),
            ('\u{0300}', 0),
            ('\u{036F}', 0),
            ('\u{0370}', 1),
            // U+00AD is explicitly NOT default-ignorable-zero in the pin.
            ('\u{00AD}', 1),
            // Conjoining jamo fillers keep their widths at the pin.
            ('\u{115F}', 2),
            ('\u{1160}', 1),
            ('\u{200B}', 0),
            ('\u{2060}', 0),
            ('\u{FE0E}', 0),
            ('\u{FE0F}', 0),
        ];
        for (scalar, expected) in BOUNDARIES {
            assert_eq!(
                model_cells(&scalar.to_string()),
                *expected,
                "boundary scalar U+{:04X} width",
                *scalar as u32
            );
        }

        // F3: a COMPOSED Hangul syllable must not absorb a following ZWJ plus
        // base. The executed oracle breaks there (the medial/final jamo is not
        // Extended_Pictographic), so the syllable and the following emoji are
        // two clusters: 2 + 2 = 4.
        const F3: &[(&str, &str, usize)] = &[
            ("f3-hangul-zwj-rocket", "\u{1100}\u{1161}\u{200D}\u{1F680}", 4),
            (
                "f3-hangul-lvt-zwj-rocket",
                "\u{1100}\u{1161}\u{11A8}\u{200D}\u{1F680}",
                4,
            ),
            ("f3-hangul-only", "\u{1100}\u{1161}", 2),
            ("f3-rocket-only", "\u{1F680}", 2),
            ("f3-zwj-rocket", "\u{200D}\u{1F680}", 2),
        ];
        for (id, text, expected) in F3 {
            assert_eq!(
                model_cells(text),
                *expected,
                "decomposed-Hangul fixture {id} must match the pinned oracle"
            );
        }

        // The subdivision flag is one cluster in BOTH segmentation paths, so
        // measurement (`fit`) and native styling (`visit_cells`) cannot split it.
        let england = FLAGS[0].1;
        let mut glyphs: Vec<(String, usize)> = Vec::new();
        let used = visit_unpadded_cells(england, 4096, |glyph, cells| {
            glyphs.push((glyph.to_string(), cells))
        });
        assert_eq!(used, 2, "the flag occupies one 2-cell cluster");
        assert_eq!(glyphs.len(), 1, "the flag is never split into pieces");
        assert_eq!(glyphs[0].0, england, "every tag scalar is retained");
        let mut first: Option<(u16, String, u16)> = None;
        visit_cells(england, 40, |x, glyph, cells| {
            if first.is_none() {
                first = Some((x, glyph.to_string(), cells));
            }
        });
        assert_eq!(
            first,
            Some((0, england.to_string(), 2)),
            "visit_cells agrees with fit on the flag cluster"
        );

        // Boundedness: an arbitrarily long tag run stays a bounded 2-cell
        // cluster. The oracle keeps it at 2; the model caps retained
        // continuations at MAX_CONTINUATIONS, which is width-invisible here.
        let long = format!("\u{1F3F4}{}", "\u{E0067}".repeat(200));
        assert_eq!(model_cells(&long), 2, "overlong tag run stays two cells");
        let fitted = fit(&long, 40);
        assert_eq!(model_cells(&fitted), 40, "composed row stays one viewport wide");
        assert!(fitted.starts_with('\u{1F3F4}'), "base is never dropped");
        assert!(
            fitted.chars().filter(|c| *c == '\u{E0067}').count() <= MAX_CONTINUATIONS,
            "retained tag scalars stay bounded"
        );
        assert!(fitted.len() <= 40 * MAX_CLUSTER_BYTES + 40);
    }

    // -----------------------------------------------------------------------
    // UI-PARITY-CELL-WIDTH-20261113 supplementary repair — focused F1 test.
    //
    // This isolates ONE fix: the missing default-ignorable plane-14 range
    // `U+E0000..=U+E0FFF` in `ZERO_WIDTH_RANGES`. Every expectation is an
    // executed full-codespace oracle value from
    // `supplementary-repair/probe-supplementary.json` (pinned npm
    // `@opentui/core-darwin-arm64@0.6.8`, dylib
    // `a2013c73d6bc9835aa59c7aa4ae6d58b0a1b6e0822a7f44ef9e21644bac72910`),
    // where the ONLY non-one range above `U+3FFFD` is exactly that range.
    // It is kept separate from the broad test so the range regression has its
    // own executed RED when that one row is removed.
    // -----------------------------------------------------------------------
    #[test]
    fn ui_parity_cell_width_supplementary_plane14_zero_range() {
        // Zero scalars: every one of them is measured 0 in the executed sweep.
        const ZERO: &[char] = &[
            '\u{E0000}', // LANGUAGE TAG
            '\u{E0001}', // TAG + deprecated markup
            '\u{E001F}',
            '\u{E0020}', // first tag letter scalar
            '\u{E007F}', // CANCEL TAG, closes every RGI subdivision flag
            '\u{E0080}',
            '\u{E00FF}',
            '\u{E0100}', // first ideographic variation selector (VS17)
            '\u{E01EF}', // last ideographic variation selector (VS256)
            '\u{E01F0}', // unassigned inside the zero span
            '\u{E0FFF}', // last zero scalar on the plane
        ];
        for scalar in ZERO {
            assert_eq!(
                model_cells(&scalar.to_string()),
                0,
                "plane-14 scalar U+{:04X} must measure zero",
                *scalar as u32
            );
        }

        // One scalar past the range is NOT zero: the oracle reports 1 for the
        // whole remainder of the plane, so the range end is exactly E0FFF.
        const ONE: &[char] = &[
            '\u{E1000}',
            '\u{E1FFF}',
            '\u{EFFFF}',
            '\u{F0000}',
            '\u{10FFFF}',
        ];
        for scalar in ONE {
            assert_eq!(
                model_cells(&scalar.to_string()),
                1,
                "scalar U+{:04X} beyond the plane-14 zero range must measure one",
                *scalar as u32
            );
        }

        // Tag scalars and variation selectors are cluster continuations, which
        // is what keeps a subdivision flag (tag base + tags + cancel) and an
        // ideographic variation sequence in one cluster.
        for scalar in ['\u{E0020}', '\u{E0067}', '\u{E007F}', '\u{E0100}', '\u{E01EF}'] {
            assert!(
                is_continuation(scalar),
                "U+{:04X} must extend the preceding cluster",
                scalar as u32
            );
        }
        assert_eq!(model_cells("\u{1F3F4}\u{E0067}\u{E007F}"), 2);
        assert_eq!(model_cells("\u{6F22}\u{E0100}"), 2);
    }

    // -----------------------------------------------------------------------
    // UI-PARITY-CELL-WIDTH-20261113 supplementary repair — focused F3 test.
    //
    // This isolates ONE fix: the `!cluster.composed` guard on the post-ZWJ join
    // in both segmentation paths. The executed oracle breaks after a COMPOSED
    // Hangul syllable (`1100 1161 200D 1F680` = 4) because conjoining jamo are
    // not Extended_Pictographic, while ordinary emoji ZWJ sequences still join
    // (`1F469 200D 1F4BB` = 2).
    // -----------------------------------------------------------------------
    #[test]
    fn ui_parity_cell_width_supplementary_composed_hangul_zwj_join() {
        // Composed Hangul syllable + ZWJ + base: two clusters, 2 + 2.
        const SPLIT: &[(&str, &str, usize)] = &[
            ("f3-hangul-zwj-rocket", "\u{1100}\u{1161}\u{200D}\u{1F680}", 4),
            (
                "f3-hangul-lvt-zwj-rocket",
                "\u{1100}\u{1161}\u{11A8}\u{200D}\u{1F680}",
                4,
            ),
            ("hangul-zwj-hangul", "\u{1100}\u{1161}\u{200D}\u{1100}\u{1161}", 4),
        ];
        // NOT asserted here: `\u{AC00}\u{200D}\u{1F680}` (a PRECOMPOSED syllable
        // followed by ZWJ) measures 2 in this model while the executed oracle
        // reports 4. That is a pre-existing gap of the same class — the model
        // does not track Extended_Pictographic, so any ZWJ after a
        // non-Extended_Pictographic base still joins. It predates this repair
        // (base behaviour is also 2) and fixing it needs a real
        // Extended_Pictographic property, which this bounded heuristic does not
        // carry. Recorded as a remaining gap in the handoff, not frozen as an
        // expected value here.
        for (id, text, expected) in SPLIT {
            assert_eq!(
                model_cells(text),
                *expected,
                "composed-Hangul fixture {id} must break at the ZWJ"
            );
        }

        // The pieces alone, so the split is 2 + 2 and not some other sum.
        assert_eq!(model_cells("\u{1100}\u{1161}"), 2);
        assert_eq!(model_cells("\u{1100}\u{1161}\u{11A8}"), 2);
        assert_eq!(model_cells("\u{200D}\u{1F680}"), 2);
        assert_eq!(model_cells("\u{1F680}"), 2);

        // The guard must not stop ordinary emoji ZWJ sequences from joining.
        assert_eq!(
            model_cells("\u{1F469}\u{200D}\u{1F4BB}"),
            2,
            "woman technologist is one 2-cell cluster"
        );
        assert_eq!(
            model_cells("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}"),
            2,
            "family ZWJ sequence is one 2-cell cluster"
        );

        // Both segmentation paths agree: `fit` emits two 2-cell clusters, and
        // `visit_cells`/`visit_unpadded_cells` place the rocket at column 2.
        let text = "\u{1100}\u{1161}\u{200D}\u{1F680}";
        let fitted = fit(text, 80);
        assert_eq!(model_cells(&fitted), 80, "the row is padded to the budget");
        assert!(
            fitted.starts_with("\u{1100}\u{1161}"),
            "the syllable is preserved intact"
        );
        assert!(
            fitted.contains('\u{1F680}'),
            "the base after the ZWJ is emitted as its own cluster"
        );

        let mut unpadded: Vec<(String, usize)> = Vec::new();
        let used = visit_unpadded_cells(text, 80, |glyph, cells| {
            unpadded.push((glyph.to_string(), cells))
        });
        assert_eq!(used, 4, "still four cells in total");
        assert_eq!(unpadded.len(), 2, "measurement sees two clusters");
        assert_eq!(unpadded[0], ("\u{1100}\u{1161}".to_string(), 2));
        assert_eq!(unpadded[1], ("\u{1F680}".to_string(), 2));

        let mut cells: Vec<(u16, String, u16)> = Vec::new();
        // Exact viewport: `visit_cells` walks the padded `fit` row, so a budget
        // equal to the cluster width leaves no padding cells to filter.
        visit_cells(text, 4, |x, glyph, w| {
            cells.push((x, glyph.to_string(), w))
        });
        assert_eq!(
            cells,
            vec![
                (0, "\u{1100}\u{1161}".to_string(), 2),
                (2, "\u{1F680}".to_string(), 2),
            ],
            "styling path splits at the same boundary as measurement"
        );
    }
}
