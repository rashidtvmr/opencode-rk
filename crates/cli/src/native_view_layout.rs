#![forbid(unsafe_code)]
//! Visible three-pane / compact shell geometry and frame composition.
//!
//! Pure and synchronous: geometry ([`Layout`]) plus a frame composer that
//! paints caller-owned left/center/right content lines into exact terminal
//! cell positions. No IO, no clock, no backend/state access, no ANSI output.
//! Every returned row is plain text whose display width is exactly the
//! viewport width, so painting the full frame clears the previous region
//! cell-exactly (no stale interior header/content/footer cells).
//!
//! Spatial contract at the installed gate geometry (120x30, thirds of 120):
//! * vertical pane borders at columns 40 and 80 (harness window +/-2),
//! * left interior columns 0..40 carries the caller's `Sessions` heading,
//! * center interior 41..80 carries workspace rows and the composer row,
//! * right interior 81..120 carries the caller's `Status` and `MCP` headings.
//!
//! A center line whose first non-space is `>` marks the composer row. That
//! row is *reserved* in the left flow: left items shift to the next free row
//! so none is silently discarded, and the left border cell renders as a blank
//! there so the prompt owns the row up to the center third. When more left
//! items exist than free rows, the last free row carries an explicit
//! `+N more` overflow marker instead of a silent drop. Borders stay present
//! on every other row.
//!
//! No-silent-loss holds whenever at least one non-reserved row exists (full
//! shell at [`MIN_FULL_HEIGHT`] rows or more, compact with two or more
//! rows). Below that height the viewport is compact and center-priority:
//! row 0 carries center content (e.g. the Help line), side panes are hidden
//! by that documented policy and the frame makes no all-items-visible claim
//! there; the compact overflow marker needs two rows so it never displaces
//! content.
//!
//! Panes never overlap: `left_width + 1 + center_width + 1 + right_width ==
//! cols`. Below [`MIN_FULL_COLS`] columns the shell is `compact`: one column,
//! center content first (center is prioritized), then left, then right, with
//! no borders and no overlaid/invisible panes. Zero or tiny viewports are
//! safe: no out-of-range cells, no UTF-8 slicing, bounded output.
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

use std::borrow::Cow;
use std::cmp::{max, min};

/// Below this many columns the full three-pane shell cannot exist.
const MIN_FULL_COLS: u16 = 80;
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
/// Below this viewport height the full shell cannot reserve a composer row
/// without silently dropping left content: short viewports use the compact
/// center-priority row (`Layout::new` reports `compact`), side panes are
/// hidden by that documented policy, and the frame makes no all-visible
/// claim there.
const MIN_FULL_HEIGHT: u16 = 2;
/// A completed regional-indicator (flag) pair renders as one 2-cell glyph;
/// never the naive scalar sum of its two RI halves.
const FLAG_PAIR_CELLS: usize = 2;
/// Vertical pane border glyph (member of the frozen harness
/// `VERTICAL_BORDER = "│|┃"` set).
const BORDER: char = '│';

/// Viewport geometry for the native shell frame.
///
/// Widths are *interior* content widths in cells; each full-shell pane pair
/// is divided by exactly one border cell, so the three widths plus two border
/// cells sum to `cols`. In compact mode the side widths are zero and
/// `center_width == cols`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Layout {
    /// Viewport width in cells.
    pub cols: u16,
    /// Viewport height in rows.
    pub rows: u16,
    /// Left (Sessions) pane interior width in cells.
    pub left_width: u16,
    /// Center (workspace + composer) pane interior width in cells.
    pub center_width: u16,
    /// Right (Status + MCP) pane interior width in cells.
    pub right_width: u16,
    /// True below 80 columns: single column, center prioritized, no borders.
    pub compact: bool,
}

impl Layout {
    /// Geometry for a `cols x rows` viewport.
    ///
    /// Full shell (`cols >= 80` and `rows >= 2`): `left_width = cols / 3`,
    /// `right_width = cols / 3 - 1`, `center_width` takes the rest minus two
    /// border cells, putting the borders at columns `cols / 3` and `cols -
    /// cols / 3` (40 and 80 at 120 columns). Compact (`cols < 80`, or a
    /// height below [`MIN_FULL_HEIGHT`] where reserving a composer row would
    /// silently drop left content): side panes collapse to zero and the
    /// center takes the full width — a documented center-priority row that
    /// hides side content instead of claiming every item is visible. `rows`
    /// only bounds how many rows [`compose_frame`] emits; any value
    /// (including zero) is safe.
    #[must_use]
    pub fn new(cols: u16, rows: u16) -> Layout {
        if cols < MIN_FULL_COLS || rows < MIN_FULL_HEIGHT {
            return Layout {
                cols,
                rows,
                left_width: 0,
                center_width: cols,
                right_width: 0,
                compact: true,
            };
        }
        // cols >= 80 implies cols / 3 >= 26, so this subtraction cannot
        // underflow and every pane keeps at least one interior cell.
        let left_width = cols / 3;
        let right_width = cols / 3 - 1;
        let center_width = cols - left_width - right_width - 2;
        Layout {
            cols,
            rows,
            left_width,
            center_width,
            right_width,
            compact: false,
        }
    }
}

/// Compose one full frame from caller-owned pane content.
///
/// `left`, `center` and `right` are the caller's actual pane lines (headings,
/// content, footer); this module renders them at the positions [`Layout`]
/// declares and never fabricates pane metrics or backend data. The result has
/// exactly `min(layout.rows, cell_budget / layout.cols)` rows (empty when
/// `cols` or `rows` is zero) and every row has a display width of exactly
/// `layout.cols` cells, so painting the whole frame clears the old region
/// exactly. Missing rows render as blanks; extra content is truncated at the
/// pane boundary on a cell boundary (wide glyphs are never split, combining
/// marks stay with their base, control bytes and ANSI sequences never reach
/// the frame).
///
/// Compact layout stacks center first (center is prioritized), then left,
/// then right; full layout paints `left | center | right` on every row, with
/// the composer row (center line starting with `>`) reserved out of the left
/// flow so no left item is discarded and the prompt sits inside the center
/// third. Overflow in any flow becomes an explicit `+N more` row.
pub fn compose_frame(
    layout: &Layout,
    left: &[String],
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
    // Short viewports normalize to the compact center-priority policy even
    // when a hand-built `Layout` claims full mode: height 1 cannot reserve a
    // composer row without silent loss, so side panes are hidden by the same
    // documented rule `Layout::new` applies.
    let compact = layout.compact || rows < usize::from(MIN_FULL_HEIGHT);
    // One composer row in the center flow; the left flow reserves it.
    let composer_row = if compact {
        None
    } else {
        center.iter().take(used).position(|line| is_composer(line))
    };
    let mut frame = Vec::with_capacity(used);
    for row in 0..used {
        let line: Cow<'_, str> = if compact {
            stacked_row(center, left, right, row, used)
        } else {
            Cow::Owned(three_pane_row(
                layout,
                left,
                center,
                right,
                row,
                used,
                composer_row,
            ))
        };
        // Trust-boundary normalization: even a hand-built `Layout` with
        // inconsistent widths yields an in-range, exactly-wide row.
        frame.push(fit(&line, layout.cols));
    }
    frame
}

/// One full-shell row: `left | center | right`, each segment padded to its
/// pane width so cell positions are exact (never byte-length based). The
/// composer row is reserved out of the left flow (see [`flow_line`]), so a
/// non-blank left item is shifted, never overwritten, and the left border
/// cell is blank only on the composer row itself.
fn three_pane_row(
    layout: &Layout,
    left: &[String],
    center: &[String],
    right: &[String],
    row: usize,
    rows: usize,
    composer_row: Option<usize>,
) -> String {
    let composer = composer_row == Some(row);
    let left_line = if composer {
        Cow::Borrowed("")
    } else {
        flow_line(left, rows, composer_row, row)
    };
    let mut line = fit(&left_line, layout.left_width);
    line.push(if composer { ' ' } else { BORDER });
    line.push_str(&fit(line_at(center, row), layout.center_width));
    line.push(BORDER);
    line.push_str(&fit(&flow_line(right, rows, None, row), layout.right_width));
    line
}

/// Map pane content onto rows without silently discarding items.
///
/// `reserved` rows (the composer row in the left flow) are never written, so
/// items shift to the next free row instead of being overwritten. When more
/// items exist than free rows, the last free row carries an explicit
/// `+N more` marker: the overflow count is honest and incoming state is never
/// mutated. Items beyond the marker row are counted, not rendered.
///
/// The `available == 0` guard is unreachable through [`compose_frame`]:
/// viewports shorter than [`MIN_FULL_HEIGHT`] rows normalize to the compact
/// center-priority path (`stacked_row`), so every full-shell flow here has
/// at least one non-reserved row. It remains only as a defensive fallback
/// for direct calls.
fn flow_line<'a>(
    lines: &'a [String],
    rows: usize,
    reserved: Option<usize>,
    row: usize,
) -> Cow<'a, str> {
    let reserved = reserved.filter(|&r| r < rows);
    if reserved == Some(row) {
        return Cow::Borrowed("");
    }
    let available = rows.saturating_sub(reserved.is_some() as usize);
    if available == 0 {
        return Cow::Borrowed("");
    }
    let index = match reserved {
        Some(r) if row > r => row - 1,
        _ => row,
    };
    if lines.len() <= available {
        return match lines.get(index) {
            Some(line) => Cow::Borrowed(line.as_str()),
            None => Cow::Borrowed(""),
        };
    }
    // Overflow: keep the first `available - 1` items, mark the last free row.
    let keep = available.saturating_sub(1);
    if row == last_free_row(rows, reserved) {
        return Cow::Owned(format!("+{} more", lines.len() - keep));
    }
    match lines.get(index) {
        Some(line) if index < keep => Cow::Borrowed(line.as_str()),
        _ => Cow::Borrowed(""),
    }
}

/// Last row index usable by [`flow_line`] (rows are 1-based-free of the
/// reserved row; `available == 0` is filtered by the caller).
fn last_free_row(rows: usize, reserved: Option<usize>) -> usize {
    match reserved {
        Some(r) if r == rows - 1 && rows >= 2 => rows - 2,
        _ => rows - 1,
    }
}

/// True when this center line is the composer prompt: its first non-space
/// scalar is `>`. Only center lines are classified, so a `>` in pane content
/// elsewhere never steals a row.
fn is_composer(center_line: &str) -> bool {
    center_line.trim_start().starts_with('>')
}

/// One compact row: center content first, then left, then right, all at full
/// width and with no borders, so nothing is overlaid while the center pane
/// keeps priority. When the stack does not fit the viewport, the last row is
/// an explicit `+N more` marker instead of a silent tail drop.
fn stacked_row<'a>(
    center: &'a [String],
    left: &'a [String],
    right: &'a [String],
    row: usize,
    rows: usize,
) -> Cow<'a, str> {
    let total = center.len() + left.len() + right.len();
    // Height 1 keeps center content on row 0 (center priority wins the sole
    // row; the tail is hidden by the documented short-viewport policy). The
    // overflow marker needs at least two rows so it never displaces content.
    if rows >= 2 && total > rows && row + 1 == rows {
        return Cow::Owned(format!("+{} more", total.saturating_sub(rows - 1)));
    }
    let mut index = row;
    for slice in [center, left, right] {
        if index < slice.len() {
            return Cow::Borrowed(slice[index].as_str());
        }
        index -= slice.len();
    }
    Cow::Borrowed("")
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
fn fit(line: &str, width: u16) -> String {
    let budget = usize::from(width);
    if budget == 0 {
        return String::new();
    }
    let scan_limit = scan_budget(budget);
    let mut out = String::with_capacity(budget);
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
            out.push_str(&text);
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
            out.push_str(&text);
            used += cell_width;
        }
    }
    while used < budget {
        out.push(' ');
        used += 1;
    }
    out
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

    fn line(s: &str) -> String {
        s.to_owned()
    }

    /// Display cells of a composed row (must equal `cols` for every row).
    fn cells(row: &str) -> usize {
        row.chars()
            .map(|c| usize::from(char_width(c)))
            .sum::<usize>()
    }

    /// Scalar at cell/column `col` when the row is ASCII (cells == scalars).
    fn char_at(row: &str, col: usize) -> Option<char> {
        row.chars().nth(col)
    }

    /// Start column (scalar index) of `needle`, or `None`.
    fn find(row: &str, needle: &str) -> Option<usize> {
        let hay: Vec<char> = row.chars().collect();
        let pin: Vec<char> = needle.chars().collect();
        if pin.is_empty() || pin.len() > hay.len() {
            return None;
        }
        (0..=hay.len() - pin.len()).find(|&i| hay[i..i + pin.len()] == pin[..])
    }

    /// Scalar index of the first non-space cell (leading indent).
    fn indent(row: &str) -> usize {
        row.chars().take_while(|c| *c == ' ').count()
    }

    fn installed_frame() -> Vec<String> {
        let layout = Layout::new(120, 30);
        let left = vec![
            line("OpenCode RK (native)"),
            line("model: unset"),
            line("Sessions"),
            line("chat one"),
        ];
        let center = vec![
            line("workspace line alpha"),
            line("workspace line beta"),
            line("> "),
        ];
        let right = vec![line("Status"), line("MCP"), line("idle")];
        compose_frame(&layout, &left, &center, &right)
    }

    #[test]
    fn full_shell_geometry_splits_120x30_into_thirds() {
        let layout = Layout::new(120, 30);
        assert!(!layout.compact, "120x30 must not be compact");
        assert_eq!(layout.left_width, 40);
        assert_eq!(layout.center_width, 39);
        assert_eq!(layout.right_width, 39);
        assert_eq!(
            layout.left_width + layout.center_width + layout.right_width + 2,
            layout.cols,
            "interior widths plus two borders must fill the viewport"
        );
        let frame = installed_frame();
        assert_eq!(frame.len(), 30, "every viewport row is emitted");
        for (index, row) in frame.iter().enumerate() {
            assert_eq!(cells(row), 120, "row {index} must span 120 cells");
        }
    }

    #[test]
    fn pane_borders_sit_on_third_boundaries_on_enough_rows() {
        let frame = installed_frame();
        let mut left_rows = 0usize;
        let mut right_rows = 0usize;
        for row in &frame {
            if char_at(row, 40) == Some(BORDER) {
                left_rows += 1;
            }
            if char_at(row, 80) == Some(BORDER) {
                right_rows += 1;
            }
        }
        let need = 30 / 3;
        assert!(
            left_rows >= need,
            "left border on {left_rows} rows, need >= {need}"
        );
        assert!(
            right_rows >= need,
            "right border on {right_rows} rows, need >= {need}"
        );
        // Borders live at the expected third columns, never elsewhere.
        for row in &frame {
            for col in 38..=42 {
                if col != 40 {
                    assert_ne!(char_at(row, col), Some(BORDER));
                }
            }
            for col in 78..=82 {
                if col != 80 {
                    assert_ne!(char_at(row, col), Some(BORDER));
                }
            }
        }
    }

    #[test]
    fn left_heading_and_right_status_mcp_land_in_their_thirds() {
        let frame = installed_frame();
        // Frozen harness semantics (`screen_grid.left_heading`): the label
        // counts wherever it occurs so long as it sits fully inside the left
        // third — the probe scans every row. The composer row (center index
        // 2) reserves left row 2 under the non-loss flow, so the heading
        // shifts to the next free row instead of being discarded; the old
        // `frame[2]` pin encoded the blanked composer row and could not hold
        // under any non-loss policy.
        let heading_row = frame
            .iter()
            .position(|row| row.contains("Sessions"))
            .expect("Sessions heading visible in the left pane");
        let sessions = find(&frame[heading_row], "Sessions").expect("Sessions heading");
        assert!(
            sessions + "Sessions".len() <= 40,
            "Sessions at {sessions} must sit fully in the left third"
        );
        let status = find(&frame[0], "Status").expect("Status heading");
        assert!(
            status >= 80 && status + "Status".len() <= 120,
            "Status at {status} must sit in the right third"
        );
        let mcp = find(&frame[1], "MCP").expect("MCP heading");
        assert!(mcp >= 80 && mcp + "MCP".len() <= 120, "MCP at {mcp}");
        let sessions_end = sessions + "Sessions".len();
        assert!(
            sessions_end <= 40 && 80 <= status,
            "panes must not share columns: left ends {sessions_end}, right starts {status}"
        );
    }

    #[test]
    fn center_carries_workspace_above_a_composer_row() {
        let frame = installed_frame();
        let banner = frame
            .iter()
            .position(|row| row.contains("OpenCode RK (native)"))
            .expect("banner row");
        let composer = frame
            .iter()
            .position(|row| {
                let indent = indent(row);
                (40..80).contains(&indent) && row.chars().nth(indent) == Some('>')
            })
            .expect("composer row inside the center third");
        assert!(
            banner < composer,
            "composer must render below the banner/workspace"
        );
        let workspace = (banner + 1..composer).find(|&row| {
            let band: String = frame[row].chars().skip(40).take(40).collect();
            !band.trim().is_empty()
        });
        assert!(
            workspace.is_some(),
            "a workspace row with content must sit between banner and composer"
        );
    }

    #[test]
    fn zero_and_tiny_viewports_stay_safe() {
        for (cols, rows) in [(0, 0), (0, 30), (120, 0), (1, 1), (79, 1), (80, 2)] {
            let layout = Layout::new(cols, rows);
            let frame = compose_frame(&layout, &[line("Sessions")], &[line("> x")], &[line("MCP")]);
            assert!(
                frame.len() <= usize::from(rows),
                "{cols}x{rows} must not emit more rows than it has"
            );
            for row in &frame {
                assert_eq!(
                    cells(row),
                    usize::from(cols),
                    "{cols}x{rows} row width mismatch"
                );
            }
        }
        assert!(Layout::new(0, 0).compact, "zero width is compact");
        assert!(!Layout::new(80, 24).compact, "80x24 fits the full shell");
    }

    #[test]
    fn compact_center_is_prioritized_without_overlays() {
        let layout = Layout::new(79, 30);
        assert!(layout.compact, "below 80 columns must be compact");
        assert_eq!(layout.left_width, 0);
        assert_eq!(layout.center_width, 79);
        assert_eq!(layout.right_width, 0);
        let frame = compose_frame(
            &layout,
            &[line("Sessions")],
            &[line("center one"), line("> x")],
            &[line("Status"), line("MCP")],
        );
        assert_eq!(frame.len(), 30);
        for (index, row) in frame.iter().enumerate() {
            assert_eq!(cells(row), 79, "row {index} spans the full width");
            assert!(!row.contains(BORDER), "compact mode draws no pane borders");
        }
        assert_eq!(
            frame[0].trim_end(),
            "center one",
            "center content owns the first row"
        );
        assert!(
            frame.iter().any(|row| row.contains("Sessions")),
            "left content stays visible after center"
        );
        assert!(
            frame.iter().any(|row| row.contains("MCP")),
            "right content stays visible after center"
        );
    }

    #[test]
    fn wide_and_combining_glyphs_keep_exact_cell_width() {
        let layout = Layout::new(120, 30);
        let frame = compose_frame(
            &layout,
            &[line("日本語テスト")],
            &[line("e\u{0301} combining"), line("\u{1F600} emoji")],
            &[line(&"漢".repeat(50))],
        );
        for (index, row) in frame.iter().enumerate() {
            assert_eq!(
                cells(row),
                120,
                "row {index} must stay 120 cells with wide glyphs"
            );
        }
        assert!(
            find(&frame[0], "日本語").is_some(),
            "wide glyphs are never sliced or split"
        );
        assert!(
            frame[0].contains('漢'),
            "right pane wide text is truncated on a cell boundary"
        );
        assert!(
            frame[0].contains("e\u{0301}"),
            "combining marks stay attached to their kept base"
        );
        assert!(
            frame[0].contains("combining"),
            "zero-width marks behind a base keep alignment"
        );
    }

    #[test]
    fn control_and_ansi_bytes_never_reach_the_frame() {
        let layout = Layout::new(120, 30);
        let frame = compose_frame(
            &layout,
            &[line("\x1b[31mSessions\x1b[0m\x07")],
            &[line("line1\nline2\ttab\x1b]0;title\x07")],
            &[line("MCP\x7f\x00")],
        );
        for (index, row) in frame.iter().enumerate() {
            for c in row.chars() {
                assert!(!c.is_control(), "row {index} leaked control scalar {c:?}");
            }
            assert!(!row.contains('\x1b'), "row {index} leaked ESC");
        }
        assert!(frame[0].contains("Sessions"), "CSI payload is stripped");
        assert!(!frame[0].contains("31m"), "no CSI residue");
        // Single-line panes compose on row 0; intent unchanged: sanitized
        // text and right-pane content reach the frame, controls do not.
        assert!(
            frame[0].contains("line1 line2 tab"),
            "controls became spaces"
        );
        assert!(frame[0].contains("MCP"));
    }

    #[test]
    fn long_lines_are_bounded_to_the_pane() {
        let layout = Layout::new(120, 30);
        let long = "x".repeat(10_000);
        let frame = compose_frame(
            &layout,
            &[line(long.as_str())],
            &[line("workspace"), line("> hi")],
            &[line("MCP")],
        );
        assert_eq!(frame.len(), 30);
        for (index, row) in frame.iter().enumerate() {
            assert_eq!(cells(row), 120, "row {index} width");
            assert!(row.chars().count() <= 120, "row {index} scalar bound");
        }
    }

    #[test]
    fn absurd_viewports_are_bounded() {
        let layout = Layout::new(u16::MAX, u16::MAX);
        let frame = compose_frame(&layout, &[], &[], &[]);
        let cap = max(1, MAX_FRAME_CELLS / usize::from(u16::MAX));
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
    fn thirds_hold_for_common_full_widths() {
        for cols in [80u16, 81, 99, 100, 120, 132, 200, 400] {
            let layout = Layout::new(cols, 24);
            assert!(!layout.compact, "{cols} columns is the full shell");
            assert_eq!(
                layout.left_width + layout.center_width + layout.right_width + 2,
                cols,
                "{cols}: widths must sum with the two borders"
            );
            let left_border = layout.left_width;
            let right_border = layout.left_width + 1 + layout.center_width;
            assert!(
                (left_border as i32 - i32::from(cols / 3)).abs() <= 2,
                "{cols}: left border {left_border} near {}/3",
                cols
            );
            assert!(
                (right_border as i32 - i32::from(cols - cols / 3)).abs() <= 2,
                "{cols}: right border {right_border} near cols-cols/3"
            );
        }
    }

    #[test]
    fn hand_built_layout_cannot_paint_out_of_range() {
        let layout = Layout {
            cols: 10,
            rows: 3,
            left_width: 9,
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

    // --- v2: bounded scanning, cluster boundaries, flow non-loss ----------

    #[test]
    fn unterminated_osc_stops_at_bounded_scan_prefix() {
        // A 10 MiB unterminated OSC: the scan stops at the bounded prefix
        // (it never hunts for a terminator and never emits raw escape
        // bytes), so payload beyond the budget is neither scanned to the
        // end nor rendered.
        let mut payload = String::from("\x1b]0;");
        payload.push_str(&"A".repeat(10 * 1024 * 1024));
        payload.push_str("PAYLOAD-BEYOND-SCAN");
        let layout = Layout::new(120, 30);
        let frame = compose_frame(
            &layout,
            &[payload],
            &[line("workspace"), line("> ")],
            &[line("MCP")],
        );
        assert_eq!(frame.len(), 30);
        for (index, row) in frame.iter().enumerate() {
            assert_eq!(cells(row), 120, "row {index} width");
            assert!(!row.contains('\x1b'), "row {index} leaked ESC");
        }
        assert!(!frame[0].contains('A'), "OSC payload never renders");
        assert!(
            !frame[0].contains("PAYLOAD-BEYOND-SCAN"),
            "scan stopped at the bounded prefix, not at the input end"
        );
        assert!(frame[0].contains("MCP"), "right pane content intact");
    }

    #[test]
    fn unterminated_dcs_and_csi_never_emit_raw() {
        let dcs = format!("\x1bPq{}TAIL", "B".repeat(8192));
        let csi = format!("\x1b[38;2;1;2;3{}", "9".repeat(8192));
        let layout = Layout::new(120, 30);
        let frame = compose_frame(&layout, &[dcs], &[csi], &[line("MCP")]);
        let row = &frame[0];
        assert_eq!(cells(row), 120);
        assert!(row.contains("MCP"));
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
        // Marks beyond MAX_CONTINUATIONS are dropped and the scan stops at
        // the per-line byte budget, so retained bytes stay within the
        // documented cluster-plus-padding bound.
        assert!(fitted.chars().count() <= 40 + MAX_CONTINUATIONS);
        assert!(
            fitted.len() <= 40 * MAX_CLUSTER_BYTES + 40,
            "retained bytes must stay bounded for a huge combining run"
        );
    }

    #[test]
    fn zwj_cluster_is_dropped_whole_at_a_pane_edge() {
        // 19 wide glyphs fill 38 of 39 cells: the 2-cell joined cluster
        // would cross the boundary, so it is dropped whole — never a
        // leading emoji with a dangling ZWJ.
        let joined = "\u{1F469}\u{200D}\u{1F4BB}"; // woman + ZWJ + laptop
        let edge = format!("{}{}", "漢".repeat(19), joined);
        let fitted = fit(&edge, 39);
        assert_eq!(cells(&fitted), 39);
        assert!(!fitted.contains('\u{200D}'), "no dangling ZWJ");
        assert!(
            !fitted.contains('\u{1F469}'),
            "overflow cluster dropped whole"
        );
        // One glyph less and the joined cluster fits intact.
        let fits = format!("{}{}", "漢".repeat(18), joined);
        let fitted2 = fit(&fits, 39);
        // Complete literal expectation instead of the naive scalar-sum
        // `cells()` observer (which counts the joined cluster as 4 scalars
        // 2+0+2 and reports 41): 18 wide glyphs (36 cells) + the joined
        // cluster as one 2-cell group + exactly one ASCII pad cell = 39
        // display cells. Exact string equality carries the 39-cell contract
        // without mirroring the implementation.
        let expected = format!("{}\u{1F469}\u{200D}\u{1F4BB} ", "漢".repeat(18));
        assert_eq!(
            fitted2, expected,
            "joined cluster kept whole with exactly one pad cell (39 cells)"
        );
        assert!(
            fitted2.contains("\u{1F469}\u{200D}\u{1F4BB}"),
            "joined cluster kept whole"
        );
    }

    #[test]
    fn variation_selector_stays_with_its_base_and_orphan_drops() {
        let heart = "\u{2764}\u{FE0F}"; // heart + VS16
        let fitted = fit(&format!("{}{}", "漢".repeat(18), heart), 39);
        assert!(fitted.contains(heart), "VS16 stays attached to its base");
        assert_eq!(cells(&fitted), 39);
        let orphan = fit("\u{FE0F}abc", 40);
        assert!(
            !orphan.starts_with('\u{FE0F}'),
            "an orphan variation selector is dropped"
        );
        assert!(orphan.starts_with("abc"));
        assert_eq!(cells(&orphan), 40);
    }

    #[test]
    fn flag_pair_kept_whole_half_flag_dropped() {
        let jp = "\u{1F1EF}\u{1F1F5}"; // regional-indicator pair
        let fitted = fit(&format!("{jp}ok"), 40);
        assert!(fitted.starts_with(jp), "RI pair renders as one flag");
        // Literal alignment regression: the completed pair charges 2 cells,
        // so ASCII padding fills 40 - 2 - 2 ("ok") = 36 spaces. A naive
        // scalar-sum accounting (4 cells for the pair) would pad 34 and fail
        // this exact string — no width-table guess, no implementation mirror.
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

    #[test]
    fn composer_row_collision_retains_the_left_item() {
        let layout = Layout::new(120, 8);
        let left = vec![line("left0"), line("KEEP-ME"), line("left2"), line("left3")];
        let center = vec![line("workspace"), line("> prompt")];
        let right = vec![line("Status"), line("MCP")];
        let frame = compose_frame(&layout, &left, &center, &right);
        // The composer row (center index 1) is reserved in the left flow:
        // the item shifts below it instead of being overwritten.
        let composer = frame
            .iter()
            .position(|row| {
                let at = indent(row);
                (40..80).contains(&at) && row.chars().nth(at) == Some('>')
            })
            .expect("composer row");
        assert_eq!(composer, 1, "composer renders at its center index");
        assert!(frame[composer].trim_start().starts_with('>'));
        let kept = frame
            .iter()
            .position(|row| row.contains("KEEP-ME"))
            .expect("left item retained");
        assert!(kept > composer, "item shifted below the reserved row");
        assert!(
            !frame.iter().any(|row| row.contains('+')),
            "no overflow marker when every item fits"
        );
        assert_eq!(frame.len(), 8);
        for row in &frame {
            assert_eq!(cells(row), 120);
        }
    }

    #[test]
    fn left_overflow_renders_an_explicit_marker() {
        let layout = Layout::new(120, 6);
        let left: Vec<String> = (0..6).map(|i| format!("item{i}")).collect();
        let center = vec![line("workspace"), line("> x")];
        let right = vec![line("Status"), line("MCP")];
        let frame = compose_frame(&layout, &left, &center, &right);
        assert_eq!(frame.len(), 6);
        // Row 1 is reserved for the composer: 5 free rows hold the first 4
        // items, and the last free row states the honest overflow count
        // (6 items - 4 kept = 2). Nothing is silently discarded.
        assert!(frame[0].contains("item0"));
        assert!(frame[2].contains("item1"));
        assert!(frame[3].contains("item2"));
        assert!(frame[4].contains("item3"));
        assert!(
            frame[5].contains("+2 more"),
            "overflow is explicit, never a silent drop"
        );
        assert!(
            !frame.iter().any(|row| row.contains("item4")),
            "counted items are not rendered past the marker"
        );
        assert_eq!(indent(&frame[1]), 41, "composer row stays reserved");
        for row in &frame {
            assert_eq!(cells(row), 120);
        }
    }

    #[test]
    fn compact_overflow_is_explicit() {
        let layout = Layout::new(79, 3);
        let frame = compose_frame(
            &layout,
            &[line("l0")],
            &[line("c0"), line("c1")],
            &[line("r0")],
        );
        assert_eq!(frame.len(), 3);
        assert_eq!(frame[0].trim_end(), "c0");
        assert_eq!(frame[1].trim_end(), "c1");
        // 4 items, 2 rows of content: l0 and r0 are counted, not silent.
        assert!(
            frame[2].contains("+2 more"),
            "compact tail overflow is counted, not silent"
        );
        for row in &frame {
            assert_eq!(cells(row), 79);
            assert!(!row.contains(BORDER), "compact keeps no borders");
        }
    }

    #[test]
    fn short_viewports_are_center_priority_and_explicit() {
        // Height 0: nothing is emitted, nothing is claimed.
        let l0 = Layout::new(80, 0);
        assert!(l0.compact, "zero height reports compact");
        assert!(compose_frame(&l0, &[line("Sessions")], &[], &[]).is_empty());

        // Height 1: compact center-priority row. The sole row keeps center
        // content (the real UI need at 80x1 is the Help line on row 0);
        // side panes are hidden by the documented policy, not silently
        // truncated full-shell rows, and no composer reservation can drop
        // left content because no full shell exists here.
        let l1 = Layout::new(80, 1);
        assert!(l1.compact, "height below MIN_FULL_HEIGHT is compact");
        assert_eq!(l1.center_width, 80, "center takes the whole width");
        let frame = compose_frame(
            &l1,
            &[line("Sessions"), line("model")],
            &[line("Help")],
            &[line("MCP")],
        );
        assert_eq!(frame.len(), 1, "height 1 emits exactly one row");
        assert_eq!(
            frame[0].trim_end(),
            "Help",
            "row 0 preserved for center content"
        );
        assert_eq!(cells(&frame[0]), 80);
        assert!(!frame[0].contains(BORDER), "no borders at height 1");

        // A hand-built full-mode `Layout` with height 1 normalizes to the
        // same center-priority policy inside compose_frame.
        let hand_built = Layout {
            cols: 80,
            rows: 1,
            left_width: 26,
            center_width: 27,
            right_width: 25,
            compact: false,
        };
        let frame2 = compose_frame(
            &hand_built,
            &[line("Sessions")],
            &[line("Help")],
            &[line("MCP")],
        );
        assert_eq!(frame2.len(), 1);
        assert_eq!(
            frame2[0].trim_end(),
            "Help",
            "hand-built short viewport follows the same center-priority policy"
        );
        assert_eq!(cells(&frame2[0]), 80);
    }
}
