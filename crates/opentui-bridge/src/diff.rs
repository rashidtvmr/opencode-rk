#![forbid(unsafe_code)]
//! Unified diff model: line numbers, gutters, selection and resize (FFI-08).
//!
//! EVIDENCE / BOUNDARY
//! - `docs/ui-parity/opencode-ui-parity-kit/audit-opentui-boundary.md` §3
//!   "Diffs, scroll controllers and terminal scrollback" (ref [D1],
//!   `packages/core/src/renderables/Diff.ts:1-120` at OpenTUI pin
//!   `8d6065a3e60039d16c6ee697692dcfbc46d3c786`) states that `DiffRenderable`
//!   parses patches, owns logical lines with added/removed/context
//!   classification, gutters, and unified/split presentation; the native
//!   primitives only supply painting. The vendored library exports **no**
//!   `diff*` symbol (`nm -gU …/libopentui.dylib`), so this is the Rust
//!   replacement for that host-side layer.
//! - Presentation rules are pinned from the OpenCode consumers at pin
//!   `450e7b9`:
//!   * `packages/tui/src/component/patch-diff.tsx:57-63` — gutter
//!     `paddingLeft = 1 + gutterPadding` and
//!     `minDigits = digits(max(oldStart+oldCount-1, newStart+newCount-1))`;
//!   * `packages/tui/src/component/patch-diff.tsx:19-21,99-119,161-176` —
//!     hunk splitting, added-patch chunking at 128 lines once a file exceeds
//!     3000 lines, and viewport-driven chunk visibility with one chunk of
//!     overscan on each side;
//!   * `packages/tui/src/feature-plugins/system/diff-viewer.tsx:44-50` —
//!     `MIN_SPLIT_WIDTH = 100`: narrower layouts fall back to unified;
//!   * `packages/tui/src/feature-plugins/system/diff-viewer.tsx:1108-1116` —
//!     separate line-number colours for context/added/removed gutters.
//!   * `packages/tui/src/util/diff.ts:20-108` — `splitAddedPatch`,
//!     `splitPatchHunks` and `splitRows` (split-mode row count is
//!     `sum(max(additions, deletions))` per change run).

use crate::color::{indexed, RgbaU16, SLOT_BORDER, SLOT_FG, SLOT_MUTED, SLOT_SUCCESS, SLOT_ERROR};

/// Patch byte cap (fail closed).
pub const MAX_PATCH_BYTES: usize = 4 * 1024 * 1024;
/// Row cap per file patch (fail closed).
pub const MAX_ROWS: usize = 200_000;
/// Split view requires this width (`MIN_SPLIT_WIDTH`).
pub const SPLIT_MIN_WIDTH: u16 = 100;
/// `VIRTUAL_MIN_LINES` for added-patch chunking.
pub const VIRTUAL_MIN_LINES: usize = 3000;
/// `VIRTUAL_CHUNK_LINES` for added-patch chunking.
pub const VIRTUAL_CHUNK_LINES: usize = 128;
/// Hunk header rows occupy one line each (`patch-diff.tsx:88-96`).
pub const HUNK_HEADER_ROWS: u16 = 1;

/// Logically classified diff row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Context,
    Added,
    Removed,
}

impl RowKind {
    /// Gutter tone; drives which line-number background is used
    /// (`diff-viewer.tsx:1111-1114`).
    #[must_use]
    pub const fn tone(self) -> RowTone {
        match self {
            Self::Context => RowTone::Context,
            Self::Added => RowTone::Added,
            Self::Removed => RowTone::Removed,
        }
    }
}

/// Semantic gutter tone (theme resolves the actual colours).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowTone {
    Context,
    Added,
    Removed,
}

/// One logical diff line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffRow {
    kind: RowKind,
    old_line: Option<u32>,
    new_line: Option<u32>,
    text: String,
    hunk: u16,
    no_newline: bool,
}

impl DiffRow {
    #[must_use]
    pub const fn kind(&self) -> RowKind {
        self.kind
    }

    #[must_use]
    pub const fn old_line(&self) -> Option<u32> {
        self.old_line
    }

    #[must_use]
    pub const fn new_line(&self) -> Option<u32> {
        self.new_line
    }

    /// Line content without the unified-diff marker.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Index of the owning hunk.
    #[must_use]
    pub const fn hunk(&self) -> u16 {
        self.hunk
    }

    /// True when the source line had no trailing newline (`\ No newline…`).
    #[must_use]
    pub const fn no_newline(&self) -> bool {
        self.no_newline
    }
}

/// One `@@` hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    header: String,
    section: String,
    old_start: u32,
    old_count: u32,
    new_start: u32,
    new_count: u32,
    first_row: usize,
    end_row: usize,
}

impl Hunk {
    #[must_use]
    pub fn header(&self) -> &str {
        &self.header
    }

    /// Text after the closing `@@` (function context).
    #[must_use]
    pub fn section(&self) -> &str {
        &self.section
    }

    #[must_use]
    pub const fn old_start(&self) -> u32 {
        self.old_start
    }

    #[must_use]
    pub const fn old_count(&self) -> u32 {
        self.old_count
    }

    #[must_use]
    pub const fn new_start(&self) -> u32 {
        self.new_start
    }

    #[must_use]
    pub const fn new_count(&self) -> u32 {
        self.new_count
    }

    #[must_use]
    pub const fn first_row(&self) -> usize {
        self.first_row
    }

    #[must_use]
    pub const fn end_row(&self) -> usize {
        self.end_row
    }

    /// Last old/new line number covered by this hunk.
    #[must_use]
    pub fn last_lines(&self) -> (u32, u32) {
        (
            self.old_start.saturating_add(self.old_count.saturating_sub(1)),
            self.new_start.saturating_add(self.new_count.saturating_sub(1)),
        )
    }
}

/// Parsed single-file patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePatch {
    old_path: String,
    new_path: String,
    rows: Vec<DiffRow>,
    hunks: Vec<Hunk>,
}

/// Fail-closed parse errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffError {
    TooLarge,
    TooManyRows,
    MissingFileHeader,
    MalformedHunkHeader,
    HunkWithoutHeader,
    HunkCountMismatch { hunk: u16 },
    EmptyHunk,
}

impl core::fmt::Display for DiffError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooLarge => f.write_str("patch exceeds byte cap"),
            Self::TooManyRows => f.write_str("patch exceeds row cap"),
            Self::MissingFileHeader => f.write_str("patch has no file header"),
            Self::MalformedHunkHeader => f.write_str("malformed hunk header"),
            Self::HunkWithoutHeader => f.write_str("hunk rows before any hunk header"),
            Self::HunkCountMismatch { hunk } => {
                write!(f, "hunk {hunk} line counts do not match its header")
            }
            Self::EmptyHunk => f.write_str("hunk has no rows"),
        }
    }
}

impl std::error::Error for DiffError {}

fn strip_prefix_path(path: &str) -> String {
    let path = path.trim_end_matches('\r');
    // Drop the tab-separated timestamp some generators append.
    let path = path.split('\t').next().unwrap_or(path);
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
        .to_string()
}

fn parse_range(input: &str) -> Option<(u32, u32)> {
    let (start, count) = match input.split_once(',') {
        Some((start, count)) => (start, count),
        None => (input, "1"),
    };
    Some((start.parse().ok()?, count.parse().ok()?))
}

/// Parse `@@ -old,count +new,count @@ section`.
fn parse_hunk_header(line: &str) -> Option<(u32, u32, u32, u32, String)> {
    let rest = line.strip_prefix("@@ ")?;
    let close = rest.find(" @@")?;
    let ranges = &rest[..close];
    let section = rest[close + 3..].trim().to_string();
    let mut parts = ranges.split(' ');
    let old = parse_range(parts.next()?.strip_prefix('-')?)?;
    let new = parse_range(parts.next()?.strip_prefix('+')?)?;
    if parts.next().is_some() {
        return None;
    }
    Some((old.0, old.1, new.0, new.1, section))
}


/// Validate an open hunk, assign its end row and push it.
fn close_hunk(
    open: &mut Option<OpenHunk>,
    hunks: &mut Vec<Hunk>,
    end_row: usize,
) -> Result<(), DiffError> {
    let Some(mut state) = open.take() else {
        return Ok(());
    };
    if state.old_seen != state.hunk.old_count || state.new_seen != state.hunk.new_count {
        return Err(DiffError::HunkCountMismatch {
            hunk: hunks.len() as u16,
        });
    }
    state.hunk.end_row = end_row;
    if state.hunk.first_row == state.hunk.end_row {
        return Err(DiffError::EmptyHunk);
    }
    hunks.push(state.hunk);
    Ok(())
}

struct OpenHunk {
    hunk: Hunk,
    old_seen: u32,
    new_seen: u32,
}

impl FilePatch {
    /// Parse a unified patch for one file.
    ///
    /// Strictness is deliberate: each hunk must consume exactly the line
    /// counts its header declares, otherwise the patch is rejected instead of
    /// rendering mis-numbered gutters.
    pub fn parse(patch: &str) -> Result<Self, DiffError> {
        if patch.len() > MAX_PATCH_BYTES {
            return Err(DiffError::TooLarge);
        }
        let mut old_path = String::new();
        let mut new_path = String::new();
        let mut saw_header = false;
        let mut rows: Vec<DiffRow> = Vec::new();
        let mut hunks: Vec<Hunk> = Vec::new();
        let mut open: Option<OpenHunk> = None;

        let lines: Vec<&str> = patch.split('\n').collect();
        for (index, raw) in lines.iter().enumerate() {
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            if index + 1 == lines.len() && line.is_empty() {
                break; // trailing newline
            }
            if let Some(path) = line.strip_prefix("--- ") {
                old_path = strip_prefix_path(path);
                saw_header = true;
                continue;
            }
            if let Some(path) = line.strip_prefix("+++ ") {
                new_path = strip_prefix_path(path);
                saw_header = true;
                continue;
            }
            if line.starts_with("@@") {
                let Some((old_start, old_count, new_start, new_count, section)) =
                    parse_hunk_header(line)
                else {
                    return Err(DiffError::MalformedHunkHeader);
                };
                close_hunk(&mut open, &mut hunks, rows.len())?;
                // Hunk indices are stored as u16 on every row; refuse overflow
                // before allocating the hunk.
                u16::try_from(hunks.len()).map_err(|_| DiffError::TooManyRows)?;
                open = Some(OpenHunk {
                    hunk: Hunk {
                        header: line.to_string(),
                        section,
                        old_start,
                        old_count,
                        new_start,
                        new_count,
                        first_row: rows.len(),
                        end_row: rows.len(),
                    },
                    old_seen: 0,
                    new_seen: 0,
                });
                continue;
            }
            if line.starts_with('\\') {
                // `\ No newline at end of file` attaches to the previous row.
                if let Some(last) = rows.last_mut() {
                    last.no_newline = true;
                }
                continue;
            }
            if line.is_empty() && open.is_none() {
                continue;
            }
            let Some(state) = open.as_mut() else {
                return Err(DiffError::HunkWithoutHeader);
            };
            let (kind, text) = match line.as_bytes().first() {
                Some(b' ') => (RowKind::Context, &line[1..]),
                Some(b'+') => (RowKind::Added, &line[1..]),
                Some(b'-') => (RowKind::Removed, &line[1..]),
                // Some tools emit bare context lines; treat them as context.
                _ => (RowKind::Context, line),
            };
            let hunk = &state.hunk;
            let old_line = match kind {
                RowKind::Added => None,
                _ => {
                    state.old_seen += 1;
                    // Saturating like the legacy classifier: a crafted
                    // `@@ -4294967295,… @@` near `u32::MAX` must not panic
                    // under overflow-checks and must match the legacy paint
                    // numbers byte-for-byte (FFI-08).
                    Some(hunk.old_start.saturating_add(state.old_seen - 1))
                }
            };
            let new_line = match kind {
                RowKind::Removed => None,
                _ => {
                    state.new_seen += 1;
                    Some(hunk.new_start.saturating_add(state.new_seen - 1))
                }
            };
            rows.push(DiffRow {
                kind,
                old_line,
                new_line,
                text: text.to_string(),
                hunk: u16::try_from(hunks.len()).map_err(|_| DiffError::TooManyRows)?,
                no_newline: false,
            });
            if rows.len() > MAX_ROWS {
                return Err(DiffError::TooManyRows);
            }
        }
        close_hunk(&mut open, &mut hunks, rows.len())?;
        if !saw_header && hunks.is_empty() {
            return Err(DiffError::MissingFileHeader);
        }
        Ok(Self {
            old_path,
            new_path,
            rows,
            hunks,
        })
    }

    #[must_use]
    pub fn old_path(&self) -> &str {
        &self.old_path
    }

    #[must_use]
    pub fn new_path(&self) -> &str {
        &self.new_path
    }

    #[must_use]
    pub fn rows(&self) -> &[DiffRow] {
        &self.rows
    }

    #[must_use]
    pub fn hunks(&self) -> &[Hunk] {
        &self.hunks
    }

    /// Highest line number on either side, used for gutter width.
    ///
    /// Mirrors `patch-diff.tsx:57-63`: `max(1, max over hunks of
    /// {oldStart+oldCount-1, newStart+newCount-1})`.
    #[must_use]
    pub fn max_line_number(&self) -> u32 {
        self.hunks
            .iter()
            .flat_map(|hunk| {
                let (old, new) = hunk.last_lines();
                [old, new]
            })
            .max()
            .unwrap_or(1)
            .max(1)
    }

    /// Gutter geometry. `padding` is the extra outer padding the caller adds
    /// (`gutterPadding`); the reference always keeps one leading cell.
    #[must_use]
    pub fn gutter(&self, padding: u16) -> Gutter {
        Gutter {
            padding_left: padding.saturating_add(1),
            min_digits: digits(self.max_line_number()) as u8,
        }
    }

    /// Row index for an old-side line number.
    #[must_use]
    pub fn row_at_old_line(&self, line: u32) -> Option<usize> {
        self.rows
            .iter()
            .position(|row| row.old_line == Some(line))
    }

    /// Row index for a new-side line number.
    #[must_use]
    pub fn row_at_new_line(&self, line: u32) -> Option<usize> {
        self.rows
            .iter()
            .position(|row| row.new_line == Some(line))
    }

    /// Split-mode row count (mirrors `splitRows`, `util/diff.ts:76-108`):
    /// context lines are one row, and each run of `+`/`-` lines occupies
    /// `max(additions, deletions)` rows.
    #[must_use]
    pub fn split_row_count(&self) -> usize {
        self.split_pairs().len()
    }

    /// Split pairs: left = removed (old side), right = added (new side).
    /// Context occupies both sides of one row.
    #[must_use]
    pub fn split_pairs(&self) -> Vec<SplitRow> {
        // Pair within each hunk: runs never merge across a hunk boundary.
        self.hunks()
            .iter()
            .flat_map(|hunk| split_pairs_in(&self.rows, hunk.first_row(), hunk.end_row()))
            .collect()
    }
}

/// Pair removals and additions inside `rows[start..end]`.
///
/// Context lines occupy both sides of one row; a run of removed/added lines
/// occupies `max(removals, additions)` rows, pairing them index-wise. This is
/// the `splitRows` shape from `packages/tui/src/util/diff.ts:76-108`.
fn split_pairs_in(rows: &[DiffRow], start: usize, end: usize) -> Vec<SplitRow> {
    let mut out = Vec::new();
    let end = end.min(rows.len());
    let mut index = start;
    while index < end {
        match rows[index].kind {
            RowKind::Context => {
                out.push(SplitRow {
                    left: Some(index),
                    right: Some(index),
                });
                index += 1;
            }
            RowKind::Removed | RowKind::Added => {
                let run_start = index;
                while index < end
                    && matches!(rows[index].kind, RowKind::Removed | RowKind::Added)
                {
                    index += 1;
                }
                let removed: Vec<usize> = (run_start..index)
                    .filter(|offset| rows[*offset].kind == RowKind::Removed)
                    .collect();
                let added: Vec<usize> = (run_start..index)
                    .filter(|offset| rows[*offset].kind == RowKind::Added)
                    .collect();
                for pair in 0..removed.len().max(added.len()) {
                    out.push(SplitRow {
                        left: removed.get(pair).copied(),
                        right: added.get(pair).copied(),
                    });
                }
            }
        }
    }
    out
}

/// Split layout row: both sides reference source rows (context uses one row
/// for both sides).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitRow {
    pub left: Option<usize>,
    pub right: Option<usize>,
}

/// Gutter geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gutter {
    pub padding_left: u16,
    pub min_digits: u8,
}

impl Gutter {
    /// Total cells reserved for one side's line number plus the separator.
    #[must_use]
    pub const fn side_width(&self) -> u16 {
        self.padding_left
            .saturating_add(self.min_digits as u16)
            .saturating_add(1)
    }

    /// Format an old/new line number pair as gutter cells. Absent numbers are
    /// padded so the separator column never shifts.
    #[must_use]
    pub fn format(&self, old: Option<u32>, new: Option<u32>) -> String {
        let width = self.min_digits as usize;
        let format_one = |value: Option<u32>| match value {
            Some(number) => format!("{number:>width$}"),
            None => " ".repeat(width),
        };
        format!(
            "{}{} {} ",
            " ".repeat(self.padding_left as usize),
            format_one(old),
            format_one(new)
        )
    }
}

/// Number of decimal digits in `value` (at least 1).
#[must_use]
pub fn digits(value: u32) -> u32 {
    let mut digits = 1;
    let mut value = value;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

/// Diff theme: line-number and gutter colours
/// (`diff-viewer.tsx:1108-1116`). Values are semantic theme slots, not
/// hard-coded ANSI numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffTheme {
    pub line_number_fg: RgbaU16,
    pub context_bg: RgbaU16,
    pub added_bg: RgbaU16,
    pub removed_bg: RgbaU16,
    pub hunk_fg: RgbaU16,
    pub border: RgbaU16,
}

impl Default for DiffTheme {
    fn default() -> Self {
        Self {
            line_number_fg: indexed(SLOT_MUTED),
            context_bg: indexed(SLOT_BORDER),
            added_bg: indexed(SLOT_SUCCESS),
            removed_bg: indexed(SLOT_ERROR),
            hunk_fg: indexed(SLOT_FG),
            border: indexed(SLOT_BORDER),
        }
    }
}

impl DiffTheme {
    /// Background for a row's gutter.
    #[must_use]
    pub const fn gutter_bg(&self, tone: RowTone) -> RgbaU16 {
        match tone {
            RowTone::Context => self.context_bg,
            RowTone::Added => self.added_bg,
            RowTone::Removed => self.removed_bg,
        }
    }
}

/// Unified vs split presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Unified,
    Split,
}

/// Wrap policy. Diffs render with `wrapMode="none"` in the virtual chunk path
/// (`patch-diff.tsx:194`), so long lines are clipped, never wrapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WrapMode {
    #[default]
    None,
}

/// Selection anchor/cursor in layout coordinates (row, display column).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub row: usize,
    pub col: usize,
}

impl Position {
    #[must_use]
    pub const fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}

/// Linear text selection over a laid-out diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffSelection {
    anchor: Position,
    cursor: Position,
}

impl DiffSelection {
    #[must_use]
    pub const fn new(anchor: Position, cursor: Position) -> Self {
        Self { anchor, cursor }
    }

    /// Click: collapse the selection to a single cell.
    #[must_use]
    pub const fn point(at: Position) -> Self {
        Self {
            anchor: at,
            cursor: at,
        }
    }

    /// Drag: extend the current selection to `at`.
    #[must_use]
    pub const fn extend(self, at: Position) -> Self {
        Self {
            anchor: self.anchor,
            cursor: at,
        }
    }

    /// Normalized (start, end) in reading order.
    #[must_use]
    pub const fn normalized(self) -> (Position, Position) {
        let (a, b) = (self.anchor, self.cursor);
        if a.row < b.row || (a.row == b.row && a.col <= b.col) {
            (a, b)
        } else {
            (b, a)
        }
    }

    /// True when the cell lies inside the selection.
    #[must_use]
    pub fn contains(self, at: Position) -> bool {
        let (start, end) = self.normalized();
        if at.row < start.row || at.row > end.row {
            return false;
        }
        if start.row == end.row {
            return at.col >= start.col && at.col <= end.col;
        }
        if at.row == start.row {
            return at.col >= start.col;
        }
        if at.row == end.row {
            return at.col <= end.col;
        }
        true
    }

    /// Selected text over a laid-out document. Row terminators become `\n`;
    /// the trailing partial line has no terminator.
    #[must_use]
    pub fn text(self, layout: &DiffLayout) -> String {
        let (start, end) = self.normalized();
        let mut out = String::new();
        let start_row = start.row.min(layout.row_count().saturating_sub(1));
        let end_row = end.row.min(layout.row_count().saturating_sub(1));
        if layout.row_count() == 0 {
            return out;
        }
        for row in start_row..=end_row {
            let Some(text) = layout.row_text(row) else {
                continue;
            };
            // `Position.col` is a display/char column: `clip` truncates rows by
            // `char` and `contains` compares columns directly. Slice on char
            // boundaries, never a raw byte offset, or a multi-byte row panics.
            let char_count = text.chars().count();
            let from_col = if row == start_row {
                start.col.min(char_count)
            } else {
                0
            };
            let to_col = if row == end_row {
                end.col.min(char_count)
            } else {
                char_count
            };
            if to_col > from_col {
                let from = byte_index_of_char(&text, from_col);
                let to = byte_index_of_char(&text, to_col);
                out.push_str(&text[from..to]);
            }
            if row != end_row {
                out.push('\n');
            }
        }
        out
    }
}

/// Why an added-only patch cannot be chunked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkError {
    NotAddedOnly,
    ZeroSize,
    TooLarge,
}

/// One virtual chunk of an added-only patch (`util/diff.ts:9-14`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedPatchChunk {
    patch: String,
    first_line: u32,
    rows: usize,
}

impl AddedPatchChunk {
    #[must_use]
    pub fn patch(&self) -> &str {
        &self.patch
    }

    #[must_use]
    pub const fn first_line(&self) -> u32 {
        self.first_line
    }

    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }
}

/// Split a new-file patch into `size`-line chunks, each a valid patch with its
/// own recomputed `@@ -0,0 +start,count @@` header.
///
/// Mirrors `splitAddedPatch` (`packages/tui/src/util/diff.ts:20-46`): returns
/// `None` (here [`ChunkError::NotAddedOnly`]) for any patch with context or
/// removed lines, because those would need old/new numbers recomputed at every
/// cut. A trailing `\ No newline at end of file` stays on the last chunk.
pub fn split_added_patch(patch: &str, size: usize) -> Result<Vec<AddedPatchChunk>, ChunkError> {
    if size == 0 {
        return Err(ChunkError::ZeroSize);
    }
    if patch.len() > MAX_PATCH_BYTES {
        return Err(ChunkError::TooLarge);
    }
    let mut header: Option<(usize, usize, u32)> = None; // (byte index, header len, count)
    let mut offset = 0;
    for line in patch.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if let Some((count, rest)) = parse_added_only_header(trimmed) {
            header = Some((offset, line.len(), count));
            let _ = rest;
            break;
        }
        offset += line.len();
    }
    let Some((header_start, header_len, count)) = header else {
        return Err(ChunkError::NotAddedOnly);
    };
    let prefix = &patch[..header_start];
    let body = &patch[header_start + header_len..];
    let body = body.strip_suffix('\n').unwrap_or(body);
    let mut lines: Vec<&str> = body.split('\n').collect();
    let marker = if lines
        .last()
        .is_some_and(|line| line.starts_with("\\ No newline"))
    {
        lines.pop()
    } else {
        None
    };
    if lines.len() != count as usize || lines.iter().any(|line| !line.starts_with('+')) {
        return Err(ChunkError::NotAddedOnly);
    }
    let total = lines.len();
    let chunk_count = total.div_ceil(size);
    let mut out = Vec::with_capacity(chunk_count);
    for index in 0..chunk_count {
        let start = index * size;
        let end = (start + size).min(total);
        let slice = &lines[start..end];
        let first_line = start as u32 + 1;
        let mut chunk = format!(
            "{prefix}@@ -0,0 +{first_line},{} @@",
            slice.len()
        );
        chunk.push('\n');
        chunk.push_str(&slice.join("\n"));
        if let Some(marker) = marker {
            if end >= total {
                chunk.push('\n');
                chunk.push_str(marker);
            }
        }
        out.push(AddedPatchChunk {
            patch: chunk,
            first_line,
            rows: slice.len(),
        });
    }
    Ok(out)
}

fn parse_added_only_header(line: &str) -> Option<(u32, &str)> {
    let rest = line.strip_prefix("@@ -0,0 +1,")?;
    let (count, tail) = rest.split_once(' ')?;
    let count: u32 = count.parse().ok()?;
    tail.strip_prefix("@@")?;
    Some((count, tail))
}

/// A hunk slice for per-hunk rendering (`splitPatchHunks`,
/// `util/diff.ts:49-71`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HunkSlice {
    pub header: Option<String>,
    pub patch: String,
    pub rows: usize,
}

/// Split a patch into one slice per hunk, each carrying the file header
/// prefix. A single-hunk patch is returned unchanged.
#[must_use]
pub fn split_patch_hunks(patch: &str) -> Vec<HunkSlice> {
    let mut offset = 0usize;
    let starts: Vec<usize> = patch
        .split_inclusive('\n')
        .filter_map(|line| {
            let start = offset;
            offset += line.len();
            line.starts_with("@@ ").then_some(start)
        })
        .collect();
    if starts.len() <= 1 {
        let body = starts.first().map_or("", |start| &patch[*start..]);
        return vec![HunkSlice {
            header: starts
                .first()
                .map(|start| patch[*start..].lines().next().unwrap_or("").to_string()),
            patch: patch.to_string(),
            rows: split_rows_of(body),
        }];
    }
    let prefix = &patch[..starts[0]];
    starts
        .iter()
        .enumerate()
        .map(|(index, start)| {
            let end = starts.get(index + 1).copied().unwrap_or(patch.len());
            let body = &patch[*start..end];
            let header = body.lines().next().map(str::to_string);
            HunkSlice {
                header,
                patch: format!("{prefix}{body}"),
                rows: split_rows_of(body),
            }
        })
        .collect()
}

/// Split-mode row count for a raw hunk body (`splitRows`,
/// `util/diff.ts:76-108`).
fn split_rows_of(hunk: &str) -> usize {
    let mut lines = hunk.lines();
    let _ = lines.next(); // header
    let lines: Vec<&str> = lines.collect();
    let mut rows = 0;
    let mut index = 0;
    while index < lines.len() {
        match lines[index].chars().next() {
            // Context row (or a blank line, which some producers emit).
            None | Some(' ') => {
                rows += 1;
                index += 1;
            }
            Some('\\') => index += 1,
            Some('+') | Some('-') => {
                let (mut additions, mut deletions) = (0, 0);
                while index < lines.len() {
                    match lines[index].chars().next() {
                        Some('+') => additions += 1,
                        Some('-') => deletions += 1,
                        _ => break,
                    }
                    index += 1;
                }
                rows += additions.max(deletions);
            }
            // Unknown prefix: count it as one row and, critically, advance so a
            // malformed patch can never spin this loop forever.
            Some(_) => {
                rows += 1;
                index += 1;
            }
        }
    }
    rows
}

/// Resize outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResizeOutcome {
    pub mode: ViewMode,
    pub mode_changed: bool,
    pub previous_scroll: usize,
    pub scroll: usize,
    pub max_scroll: usize,
}

/// Laid-out diff viewport. Owns the rendered row text; the painter supplies
/// colours through [`DiffTheme`].
#[derive(Debug, Clone)]
pub struct DiffLayout {
    mode: ViewMode,
    width: u16,
    height: u16,
    scroll: usize,
    rows: Vec<RenderedRow>,
}

/// One painted layout row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedRow {
    /// Source row kind; `None` for a hunk header row.
    pub kind: Option<RowKind>,
    /// Unified text, or the joined split text (used for selection/copy).
    pub text: String,
    pub left: Option<String>,
    pub right: Option<String>,
    /// True for the `@@ … @@` header row.
    pub hunk_header: bool,
}

/// Clip a rendered row to `width` cells.
///
/// The pinned renderer clips instead of wrapping (`patch-diff.tsx:194`); the
/// model stores plain text, so clipping truncates by `char` count. Wide (CJK)
/// cells are not re-measured here — the painter owns cell measurement.
fn clip(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    text.chars().take(width).collect()
}

/// Byte offset of the `char_col`-th char boundary (the row's byte length for an
/// out-of-range column). Selection columns are char indices, matching [`clip`].
fn byte_index_of_char(text: &str, char_col: usize) -> usize {
    text.char_indices()
        .nth(char_col)
        .map_or(text.len(), |(byte, _)| byte)
}

impl DiffLayout {
    /// Build a layout for `patch` at `width`x`height`. Split mode is only
    /// used when the width reaches [`SPLIT_MIN_WIDTH`].
    #[must_use]
    pub fn new(patch: &FilePatch, mode: ViewMode, width: u16, height: u16) -> Self {
        let mut layout = Self {
            mode,
            width,
            height,
            scroll: 0,
            rows: Vec::new(),
        };
        layout.rebuild(patch);
        layout
    }

    const fn effective_mode(&self) -> ViewMode {
        match self.mode {
            ViewMode::Split if self.width < SPLIT_MIN_WIDTH => ViewMode::Unified,
            mode => mode,
        }
    }

    fn rebuild(&mut self, patch: &FilePatch) {
        self.rows.clear();
        let mode = self.effective_mode();
        let width = self.width as usize;
        for hunk in patch.hunks() {
            self.rows.push(RenderedRow {
                kind: None,
                text: clip(hunk.header(), width),
                left: None,
                right: None,
                hunk_header: true,
            });
            match mode {
                ViewMode::Unified => {
                    for row in &patch.rows()[hunk.first_row()..hunk.end_row()] {
                        let marker = match row.kind() {
                            RowKind::Context => ' ',
                            RowKind::Added => '+',
                            RowKind::Removed => '-',
                        };
                        self.rows.push(RenderedRow {
                            kind: Some(row.kind()),
                            text: clip(&format!("{marker}{}", row.text()), width),
                            left: None,
                            right: None,
                            hunk_header: false,
                        });
                    }
                }
                ViewMode::Split => {
                    let column = width.saturating_sub(3) / 2;
                    for pair in split_pairs_in(patch.rows(), hunk.first_row(), hunk.end_row()) {
                        let left = pair.left.and_then(|index| patch.rows().get(index)).map(|row| {
                            // Context cells keep two leading blanks (gutter
                            // separator + context marker) so both sides align.
                            let prefix = match row.kind() {
                                RowKind::Removed => "-",
                                RowKind::Added => "+",
                                RowKind::Context => "  ",
                            };
                            clip(&format!("{prefix}{}", row.text()), column)
                        });
                        let right = pair.right.and_then(|index| patch.rows().get(index)).map(|row| {
                            let prefix = match row.kind() {
                                RowKind::Removed => "-",
                                RowKind::Added => "+",
                                RowKind::Context => "  ",
                            };
                            clip(&format!("{prefix}{}", row.text()), column)
                        });
                        let kind = pair
                            .left
                            .or(pair.right)
                            .and_then(|index| patch.rows().get(index))
                            .map(DiffRow::kind);
                        let text = format!(
                            "{} | {}",
                            left.clone().unwrap_or_default(),
                            right.clone().unwrap_or_default()
                        );
                        self.rows.push(RenderedRow {
                            kind,
                            text,
                            left,
                            right,
                            hunk_header: false,
                        });
                    }
                }
            }
        }
        self.scroll = self.scroll.min(self.max_scroll());
    }

    #[must_use]
    pub const fn mode(&self) -> ViewMode {
        self.effective_mode()
    }

    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    #[must_use]
    pub const fn scroll(&self) -> usize {
        self.scroll
    }

    #[must_use]
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Largest valid scroll offset (no blank overscroll past the end).
    #[must_use]
    pub fn max_scroll(&self) -> usize {
        self.rows.len().saturating_sub(self.height as usize)
    }

    /// Resize the viewport. Split/unified switches at [`SPLIT_MIN_WIDTH`];
    /// the scroll offset is clamped so the viewport never leaves the document.
    pub fn resize(&mut self, patch: &FilePatch, width: u16, height: u16) -> ResizeOutcome {
        let previous_mode = self.effective_mode();
        let previous_scroll = self.scroll;
        self.width = width;
        self.height = height;
        self.rebuild(patch);
        let mode = self.effective_mode();
        let max_scroll = self.max_scroll();
        self.scroll = self.scroll.min(max_scroll);
        ResizeOutcome {
            mode,
            mode_changed: mode != previous_mode,
            previous_scroll,
            scroll: self.scroll,
            max_scroll,
        }
    }

    /// Scroll by `delta` rows, clamped to the document.
    pub fn scroll_by(&mut self, delta: i32) -> usize {
        let max = self.max_scroll();
        self.scroll = if delta.is_negative() {
            self.scroll.saturating_sub(delta.unsigned_abs() as usize)
        } else {
            self.scroll.saturating_add(delta as usize).min(max)
        };
        self.scroll
    }

    /// Visible row indices (clipped to the viewport height).
    #[must_use]
    pub fn visible(&self) -> std::ops::Range<usize> {
        let end = (self.scroll + self.height as usize).min(self.rows.len());
        self.scroll..end
    }

    /// Rendered text of one layout row.
    #[must_use]
    pub fn row_text(&self, index: usize) -> Option<String> {
        self.rows.get(index).map(|row| row.text.clone())
    }

    /// One painted layout row (unified text or split sides).
    #[must_use]
    pub fn row_line(&self, index: usize) -> Option<&RenderedRow> {
        self.rows.get(index)
    }
}

/// Visible chunk indices for a virtualized added patch
/// (`patch-diff.tsx:161-176`): one chunk of overscan on each side.
#[must_use]
pub fn visible_chunks(scroll_top: usize, viewport_height: usize, chunk_count: usize) -> std::ops::Range<usize> {
    let first = scroll_top / VIRTUAL_CHUNK_LINES;
    let last = (scroll_top + viewport_height) / VIRTUAL_CHUNK_LINES;
    let start = first.saturating_sub(1);
    let end = last.saturating_add(2).min(chunk_count);
    start.min(end)..end
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH: &str = "--- a/src/main.rs\n\
+++ b/src/main.rs\n\
@@ -1,4 +1,5 @@ fn main\n\
 use std::io;\n\
-let a = 1;\n\
+let a = 2;\n\
+let b = 3;\n\
 println!(\"hi\");\n\
 end;\n";

    fn patch() -> FilePatch {
        FilePatch::parse(PATCH).expect("valid patch")
    }

    #[test]
    fn parses_paths_hunks_and_line_numbers() {
        let patch = patch();
        assert_eq!(patch.old_path(), "src/main.rs");
        assert_eq!(patch.new_path(), "src/main.rs");
        assert_eq!(patch.hunks().len(), 1);
        let hunk = &patch.hunks()[0];
        assert_eq!((hunk.old_start(), hunk.old_count()), (1, 4));
        assert_eq!((hunk.new_start(), hunk.new_count()), (1, 5));
        assert_eq!(hunk.section(), "fn main");
        assert_eq!(hunk.last_lines(), (4, 5));
        assert_eq!(patch.rows().len(), 6);

        let kinds: Vec<RowKind> = patch.rows().iter().map(DiffRow::kind).collect();
        assert_eq!(
            kinds,
            [
                RowKind::Context,
                RowKind::Removed,
                RowKind::Added,
                RowKind::Added,
                RowKind::Context,
                RowKind::Context
            ]
        );
        // Old/new numbering advances independently of the marker.
        assert_eq!(
            patch
                .rows()
                .iter()
                .map(|row| (row.old_line(), row.new_line()))
                .collect::<Vec<_>>(),
            [
                (Some(1), Some(1)),
                (Some(2), None),
                (None, Some(2)),
                (None, Some(3)),
                (Some(3), Some(4)),
                (Some(4), Some(5)),
            ]
        );
        assert_eq!(patch.row_at_old_line(2), Some(1));
        assert_eq!(patch.row_at_new_line(3), Some(3));
        assert_eq!(patch.row_at_new_line(99), None);
    }

    #[test]
    fn gutter_matches_reference_geometry() {
        let patch = patch();
        // max line number 5 -> one digit; padding 0 keeps one leading cell.
        let gutter = patch.gutter(0);
        assert_eq!(gutter.padding_left, 1);
        assert_eq!(gutter.min_digits, 1);
        assert_eq!(gutter.side_width(), 3);
        assert_eq!(gutter.format(Some(2), None), " 2   ");
        assert_eq!(gutter.format(None, Some(3)), "   3 ");

        // A wider padding shifts both numbers right by that many cells.
        let padded = patch.gutter(2);
        assert_eq!(padded.padding_left, 3);
        assert_eq!(padded.format(Some(1), Some(1)), "   1 1 ");

        // Two-digit numbers widen the reserved field.
        let wide = FilePatch::parse(
            "--- a/x\n+++ b/x\n@@ -99,1 +99,1 @@\n-old\n+new\n",
        )
        .unwrap();
        assert_eq!(wide.gutter(0).min_digits, 2);
        assert_eq!(wide.max_line_number(), 99);
        assert_eq!(digits(0), 1);
        assert_eq!(digits(1000), 4);
    }

    #[test]
    fn no_newline_marker_attaches_to_previous_row() {
        let patch = FilePatch::parse(
            "--- a/x\n+++ b/x\n@@ -1,1 +1,1 @@\n-old\n\\ No newline at end of file\n+new\n",
        )
        .unwrap();
        assert!(patch.rows()[0].no_newline());
        assert_eq!(patch.rows()[1].text(), "new");
        assert!(!patch.rows()[1].no_newline());
    }

    #[test]
    fn hunk_count_mismatch_is_rejected() {
        let patch = "--- a/x\n+++ b/x\n@@ -1,3 +1,3 @@\n a\n b\n";
        assert_eq!(
            FilePatch::parse(patch).unwrap_err(),
            DiffError::HunkCountMismatch { hunk: 0 }
        );
        assert_eq!(
            FilePatch::parse("--- a/x\n+++ b/x\n@@ garbage @@\n a\n").unwrap_err(),
            DiffError::MalformedHunkHeader
        );
        assert_eq!(
            FilePatch::parse("+orphan\n").unwrap_err(),
            DiffError::HunkWithoutHeader
        );
        assert_eq!(
            FilePatch::parse("").unwrap_err(),
            DiffError::MissingFileHeader
        );
    }

    #[test]
    fn multiple_hunks_are_indexed() {
        let patch = FilePatch::parse(
            "--- a/x\n+++ b/x\n\
@@ -1,1 +1,1 @@ first\n-a\n+b\n\
@@ -10,1 +10,2 @@ second\n c\n+d\n",
        )
        .unwrap();
        assert_eq!(patch.hunks().len(), 2);
        assert_eq!(patch.hunks()[1].first_row(), 2);
        assert_eq!(patch.rows()[2].hunk(), 1);
        assert_eq!(patch.rows()[2].old_line(), Some(10));
        assert_eq!(patch.rows()[3].new_line(), Some(11));
    }

    #[test]
    fn split_pairs_pair_removals_with_additions() {
        let patch = patch();
        let pairs = patch.split_pairs();
        // context, removed+added pair, added-only pair, context, context
        assert_eq!(pairs.len(), 5);
        assert_eq!(pairs[1].left, Some(1));
        assert_eq!(pairs[1].right, Some(2));
        assert_eq!(pairs[2].left, None);
        assert_eq!(pairs[2].right, Some(3));
        assert_eq!(pairs[3].left, Some(4));
        assert_eq!(pairs[3].right, Some(4));
        assert_eq!(patch.split_row_count(), 5);

        // `splitRows` counts max(additions, deletions) per change run.
        let hunk = "abc\n@@ -1,3 +1,5 @@\n a\n-b\n-c\n+d\n+e\n+f\n";
        assert_eq!(split_patch_hunks(hunk).last().unwrap().rows, 4);
    }

    #[test]
    fn layout_emits_hunk_header_rows_and_clips_to_width() {
        let patch = patch();
        let layout = DiffLayout::new(&patch, ViewMode::Unified, 40, 10);
        assert_eq!(layout.mode(), ViewMode::Unified);
        // header + 6 rows
        assert_eq!(layout.row_count(), 7);
        let header = layout.row_line(0).unwrap();
        assert_eq!(header.kind, None);
        assert!(header.text.starts_with("@@ -1,4 +1,5 @@"));
        let first = layout.row_line(1).unwrap();
        assert_eq!(first.text, " use std::io;");
        let removed = layout.row_line(2).unwrap();
        assert_eq!(removed.kind, Some(RowKind::Removed));
        assert_eq!(removed.text, "-let a = 1;");
        // Width 12 clips long rows instead of wrapping.
        let long = FilePatch::parse(
            "--- a/x\n+++ b/x\n@@ -1,1 +1,1 @@\n-abcdefghijklmnop\n+abcdefghijklmnop\n",
        )
        .unwrap();
        let layout = DiffLayout::new(&long, ViewMode::Unified, 5, 10);
        assert_eq!(layout.row_line(1).unwrap().text, "-abcd");
    }

    #[test]
    fn resize_switches_split_at_the_reference_threshold() {
        let patch = patch();
        let mut layout = DiffLayout::new(&patch, ViewMode::Split, 99, 8);
        assert_eq!(layout.mode(), ViewMode::Unified, "below 100 is unified");
        let outcome = layout.resize(&patch, 100, 8);
        assert_eq!(outcome.mode, ViewMode::Split);
        assert!(outcome.mode_changed);
        let outcome = layout.resize(&patch, 99, 8);
        assert_eq!(outcome.mode, ViewMode::Unified);
        assert!(outcome.mode_changed);
        let outcome = layout.resize(&patch, 60, 8);
        assert!(!outcome.mode_changed, "already unified");
    }

    #[test]
    fn resize_clamps_scroll_and_preserves_geometry() {
        let patch = patch();
        let mut layout = DiffLayout::new(&patch, ViewMode::Unified, 80, 3);
        assert_eq!(layout.max_scroll(), 4);
        assert_eq!(layout.scroll_by(100), 4);
        let outcome = layout.resize(&patch, 80, 10);
        assert_eq!(outcome.previous_scroll, 4);
        assert_eq!(outcome.scroll, 0, "scroll clamped to the shorter document");
        assert_eq!(outcome.max_scroll, 0);
        let outcome = layout.resize(&patch, 80, 2);
        assert_eq!(outcome.max_scroll, 5);
        assert_eq!(outcome.scroll, 0);
        assert!(layout.visible().end <= layout.row_count());
        layout.scroll_by(2);
        assert_eq!(layout.visible(), 2..4);
        assert_eq!(layout.scroll_by(-10), 0);
    }

    #[test]
    fn split_layout_paints_both_sides() {
        let patch = patch();
        let layout = DiffLayout::new(&patch, ViewMode::Split, 100, 10);
        // header + 5 pairs
        assert_eq!(layout.row_count(), 6);
        let pair = layout.row_line(2).unwrap();
        assert_eq!(pair.left.as_deref(), Some("-let a = 1;"));
        assert_eq!(pair.right.as_deref(), Some("+let a = 2;"));
        let context = layout.row_line(1).unwrap();
        assert_eq!(context.left.as_deref(), Some("  use std::io;"));
        assert_eq!(context.right.as_deref(), Some("  use std::io;"));
        // Added-only pair keeps the left side empty.
        let added = layout.row_line(3).unwrap();
        assert_eq!(added.left, None);
        assert_eq!(added.right.as_deref(), Some("+let b = 3;"));
    }

    #[test]
    fn selection_spans_partial_and_multi_rows() {
        let patch = patch();
        let layout = DiffLayout::new(&patch, ViewMode::Unified, 80, 10);
        let selection = DiffSelection::new(Position::new(1, 5), Position::new(2, 6));
        assert!(selection.contains(Position::new(1, 5)));
        assert!(!selection.contains(Position::new(1, 3)));
        let text = selection.text(&layout);
        // Row 1 is " use std::io;", row 2 "-let a = 1;"; markers are part of
        // the laid-out row text, so selection copies exactly what is painted.
        assert_eq!(text, "std::io;\n-let a");
        // Click collapses to a point.
        let click = DiffSelection::point(Position::new(2, 0)).extend(Position::new(2, 3));
        assert_eq!(click.normalized().0.row, 2);
        assert_eq!(click.text(&layout), "-le");
        // Reversed drag normalizes.
        let reversed = DiffSelection::new(Position::new(4, 2), Position::new(1, 0));
        assert_eq!(reversed.normalized().0, Position::new(1, 0));
        assert!(reversed.contains(Position::new(3, 99)));
        assert_eq!(reversed.text(&layout).lines().count(), 4);
    }

    #[test]
    fn selection_clamps_out_of_range_positions() {
        let patch = patch();
        let layout = DiffLayout::new(&patch, ViewMode::Unified, 80, 10);
        let selection = DiffSelection::new(Position::new(0, 0), Position::new(999, 999));
        let text = selection.text(&layout);
        assert!(text.starts_with("@@ -1,4 +1,5 @@"));
        assert!(text.ends_with(" end;"));
        let empty = DiffSelection::new(Position::new(5, 0), Position::new(5, 0));
        assert_eq!(empty.text(&layout), "");
    }

    #[test]
    fn gutter_tones_map_to_theme_colours() {
        let theme = DiffTheme::default();
        assert_ne!(theme.gutter_bg(RowTone::Added), theme.gutter_bg(RowTone::Removed));
        assert_ne!(theme.gutter_bg(RowTone::Context), theme.gutter_bg(RowTone::Added));
        assert_eq!(RowKind::Context.tone(), RowTone::Context);
        assert_eq!(RowKind::Added.tone(), RowTone::Added);
        assert_eq!(RowKind::Removed.tone(), RowTone::Removed);
        assert_ne!(theme.line_number_fg, theme.context_bg);
        assert_eq!(theme.border, indexed(SLOT_BORDER));
    }

    #[test]
    fn split_added_patch_recomputes_headers() {
        let patch = "--- /dev/null\n+++ b/new.txt\n@@ -0,0 +1,3 @@\n+a\n+b\n+c\n";
        let chunks = split_added_patch(patch, 2).unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].first_line(), 1);
        assert_eq!(chunks[0].rows(), 2);
        assert!(chunks[0].patch().contains("@@ -0,0 +1,2 @@\n+a\n+b"));
        assert!(chunks[0].patch().starts_with("--- /dev/null\n+++ b/new.txt\n"));
        assert_eq!(chunks[1].first_line(), 3);
        assert_eq!(chunks[1].rows(), 1);
        assert!(chunks[1].patch().contains("@@ -0,0 +3,1 @@\n+c"));
        // Each chunk is itself a parseable added-only patch.
        assert_eq!(FilePatch::parse(chunks[1].patch()).unwrap().rows().len(), 1);
    }

    #[test]
    fn split_added_patch_rejects_non_added_patches() {
        assert_eq!(
            split_added_patch("@@ -1,1 +1,1 @@\n-a\n+b\n", 2),
            Err(ChunkError::NotAddedOnly)
        );
        assert_eq!(
            split_added_patch("@@ -0,0 +1,2 @@\n+a\n b\n", 2),
            Err(ChunkError::NotAddedOnly)
        );
        assert_eq!(
            split_added_patch("@@ -0,0 +1,2 @@\n+a\n+b\n", 0),
            Err(ChunkError::ZeroSize)
        );
        // A trailing no-newline marker rides on the last chunk.
        let patch = "--- /dev/null\n+++ b/x\n@@ -0,0 +1,2 @@\n+a\n+b\n\\ No newline at end of file\n";
        let chunks = split_added_patch(patch, 1).unwrap();
        assert_eq!(chunks.len(), 2);
        assert!(!chunks[0].patch().contains("No newline"));
        assert!(chunks[1].patch().ends_with("\\ No newline at end of file"));
    }

    #[test]
    fn hunk_split_preserves_file_prefix() {
        let patch = "--- a/x\n+++ b/x\n@@ -1,1 +1,1 @@ one\n-a\n+b\n@@ -5,1 +5,1 @@ two\n-c\n+d\n";
        let slices = split_patch_hunks(patch);
        assert_eq!(slices.len(), 2);
        assert_eq!(slices[0].header.as_deref(), Some("@@ -1,1 +1,1 @@ one"));
        assert!(slices[0].patch.starts_with("--- a/x\n+++ b/x\n@@ -1,1 +1,1 @@"));
        assert_eq!(slices[0].rows, 1);
        assert!(slices[1].patch.contains("@@ -5,1 +5,1 @@ two"));
        assert_eq!(slices[1].rows, 1);
        // A single-hunk patch is returned unchanged.
        assert_eq!(split_patch_hunks("@@ -0,0 +1,1 @@\n+a\n").len(), 1);
        // …and its row count ignores the file header even when one is present.
        let single = split_patch_hunks("--- a/x\n+++ b/x\n@@ -0,0 +1,1 @@\n+a\n");
        assert_eq!(single.len(), 1);
        assert_eq!(single[0].rows, 1);
    }

    #[test]
    fn virtual_chunks_overscan_one_each_side() {
        assert_eq!(visible_chunks(0, 10, 40), 0..2);
        // Mid-document: current chunk +/- one.
        assert_eq!(visible_chunks(128 * 5, 20, 40), 4..7);
        // Clamped at the end.
        assert_eq!(visible_chunks(128 * 39, 20, 40), 38..40);
        // Constants used by the reference implementation.
        assert_eq!(VIRTUAL_CHUNK_LINES, 128);
        assert_eq!(VIRTUAL_MIN_LINES, 3000);
    }

    #[test]
    fn byte_and_row_caps_fail_closed() {
        let huge = "x".repeat(MAX_PATCH_BYTES + 1);
        assert_eq!(FilePatch::parse(&huge).unwrap_err(), DiffError::TooLarge);
    }

    #[test]
    fn crafted_max_start_saturates_instead_of_panicking() {
        // FFI-08 review: `old_start + old_seen - 1` in plain u32 overflowed on
        // `@@ -4294967295,…` plus two consumed rows. The saturating form
        // matches the legacy classifier (`saturating_add` per consumed row).
        let patch = FilePatch::parse(
            "--- a/x\n+++ b/x\n@@ -4294967295,2 +1,2 @@\n a\n b\n",
        )
        .expect("parse must not panic at u32 bounds");
        assert_eq!(
            patch
                .rows()
                .iter()
                .map(|row| (row.old_line(), row.new_line()))
                .collect::<Vec<_>>(),
            [(Some(u32::MAX), Some(1)), (Some(u32::MAX), Some(2))]
        );
        assert_eq!(patch.hunks()[0].last_lines(), (u32::MAX, 2));
        assert_eq!(patch.max_line_number(), u32::MAX);
    }

    #[test]
    fn selection_copies_multibyte_rows_by_char_column() {
        // Rows are clipped by `char` (see `clip`), so `Position.col` is a char
        // column. The selection must slice on char boundaries, not byte
        // offsets, or a multi-byte row splits a code point and panics.
        let patch = FilePatch::parse(
            "--- a/e.rs\n+++ b/e.rs\n@@ -1,2 +1,2 @@\n-caf\u{e9}\n+caf\u{e9}!\n na\u{ef}ve\n",
        )
        .unwrap();
        let layout = DiffLayout::new(&patch, ViewMode::Unified, 80, 10);
        // Rows: 0 header, 1 "-caf\u{e9}", 2 "+caf\u{e9}!", 3 " na\u{ef}ve".
        let row = layout.row_text(2).unwrap();
        assert_eq!(row, "+caf\u{e9}!");
        assert_eq!(row.chars().count(), 6);
        assert_eq!(row.len(), 7, "one multi-byte code point");
        // Whole row: char cols 0..6 map to bytes 0..7.
        let whole = DiffSelection::new(Position::new(2, 0), Position::new(2, 6));
        assert_eq!(whole.text(&layout), "+caf\u{e9}!");
        // Partial span crossing the multi-byte char: cols 3..5 are "f\u{e9}".
        // Before the fix this byte-slices 3..5 and panics mid code point.
        let partial = DiffSelection::new(Position::new(2, 3), Position::new(2, 5));
        assert_eq!(partial.text(&layout), "f\u{e9}");
        assert!(partial.contains(Position::new(2, 4)));
    }
}
