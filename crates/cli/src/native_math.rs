#![forbid(unsafe_code)]
//! LaTeX / math code-fence lifecycle for the native Markdown flow (package
//! P13b).
//!
//! Scope and depth limit (explicit)
//! --------------------------------
//! This module models the **fence lifecycle only**: fence/language recognition
//! (`latex`, `math`, and the `tex` alias), info-string parsing, upstream input
//! limits (source length, brace nesting), the deterministic fallback to the
//! plain code block, last-good retention that only applies when the new source
//! *extends* the previous source, a render-output cache keyed by
//! `(source, width, theme)` and the resulting block/scroll footprint.
//!
//! It deliberately did **not** originally model LaTeX parsing or layout;
//! MD-009 adds the real producer: [`crate::native_math_engine::
//! UpstreamMathEngine`] is a bounded port of `packages/latex/src/parser.ts`
//! plus `layout.ts` that returns painted glyph [`MathCell`]s in
//! [`MathLayout::cells`]. [`MathLayoutEngine`] is still the caller-supplied
//! boundary, so the unknown-command / oversize fail-closed path is unchanged.
//! Macro expansion (`ParseOptions.macros`) is not ported: the only live
//! caller (`markdown.ts:112-118`) passes no macros.
//!
//! Evidence pins (OpenCode pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`)
//! --------------------------------------------------------------------
//! * `packages/tui/src/plugin/builtins.ts:14,30` – the Latex plugin is a
//!   builtin.
//! * `packages/latex/src/plugin.ts:7-12` – registers the same renderer for the
//!   `latex` and `math` code-fence languages.
//! * `packages/latex/src/markdown.test.ts:53` – `["latex", "math", "tex",
//!   "LATEX title=example"]` all render, i.e. the fence language is matched
//!   case-insensitively on the first info-string token and `tex` reaches the
//!   `latex` handler. The @opentui alias table itself is not vendored; this is
//!   the evidence for the alias accepted here.
//! * `packages/latex/src/limits.ts:3-4` – `DEFAULT_MAX_SOURCE_LENGTH = 100_000`
//!   and `DEFAULT_MAX_NESTING_DEPTH = 256`.
//! * `packages/latex/src/limits.ts:14-18` – source-length check.
//! * `packages/latex/src/limits.ts:20-40` – brace-nesting check: a backslash
//!   run escapes the following brace only when the run length is odd;
//!   unmatched `}` floors the depth at zero.
//! * `packages/latex/src/parser.ts:65-77` – both limits are applied to the
//!   source (and again to the macro-expanded source; this module does not
//!   expand macros, see below).
//! * `packages/latex/src/types.ts:63-71,85-91` – `LatexParseError` carries an
//!   offset; `MathLayout` is `width`, `height`, `baseline`, `cells`.
//! * `packages/latex/src/render.ts:5-7` – `renderLatex(source, options)`
//!   returns a `MathLayout`.
//! * `packages/latex/src/markdown.ts:27-51` – the renderer: `lastGood` keyed by
//!   the default-render block id (`31-35`), retention only when
//!   `token.text.startsWith(previous.source)` (`36`), fence marker from
//!   `token.raw` (`37`), streaming only for an **unclosed** fence (`38-42`),
//!   layout attempt (`43`), and the frame choice — fresh layout, else the
//!   retained frame while streaming, else the plain code block (`44-50`).
//! * `packages/latex/src/markdown.ts:53-98` – the formula is a
//!   `width: "100%"`, `minWidth: layout.width`, `height: layout.height`,
//!   `wrapMode: "none"` text renderable inside a `ScrollBoxRenderable`
//!   (`height: layout.height`, `marginTop: 1`, `scrollX: true`,
//!   `scrollY: false`); only shift-modified or horizontal wheel events are
//!   consumed, so vertical transcript scrolling is not stolen; both scrollbars
//!   are hidden (`96-97`).
//! * `packages/latex/src/markdown.ts:99-108` – `lastGood` cleanup on destroy.
//! * `packages/latex/src/markdown.ts:112-118` – layout is attempted with
//!   `{ strict: true, displayMode: true }`; `LatexParseError` and `RangeError`
//!   become `undefined` (fallback/retention), other errors propagate.
//!
//! Not modelled
//! ------------
//! * LaTeX grammar, macro expansion (`parser.ts:77` re-checks the expanded
//!   source; we only check the raw source) and all math layout/drawing.
//! * `@opentui` `CodeRenderable.streaming`: the flow supplies the streaming
//!   flag through [`MathFence`]; OC2 assumes the fallback renderable is a code
//!   block, which it is whenever a custom fence renderer declines.
//! * Upstream measures `source.length` in UTF-16 code units; this module
//!   measures Unicode scalar values. The two are identical for ASCII math and
//!   reported offsets are char indices, not UTF-16 indices.
//! * The 250 000-cell layout bound is an OC2 fail-closed bound, not upstream
//!   LaTeX behavior.
//!
//! Integration notes for `native_markdown_flow`
//! --------------------------------------------
//! Call [`MathFence::from_code_block`] with the block id, the fence language,
//! `BlockBody::Code` content joined with `\n` (upstream `token.text`), the
//! block's `source_raw` (upstream `token.raw`) and the flow's `streaming`
//! option, then [`MathRenderer::render`] with the terminal width.
//! [`MathOutcome::Fallback`] means: paint the normal code block with the
//! preserved source. [`MathOutcome::Rendered`] carries the footprint
//! ([`MathFrame::footprint_rows`]) and a [`MathScrollBox`] for horizontal
//! scrolling.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;

/// `DEFAULT_MAX_SOURCE_LENGTH` (`packages/latex/src/limits.ts:3`).
pub const MAX_MATH_SOURCE_CHARS: usize = 100_000;
/// `DEFAULT_MAX_NESTING_DEPTH` (`packages/latex/src/limits.ts:4`).
pub const MAX_MATH_NESTING_DEPTH: usize = 256;
/// Scroll-box `marginTop` (`packages/latex/src/markdown.ts:86`).
pub const MATH_MARGIN_TOP: usize = 1;
/// OC2 fail-closed bound on a laid-out formula (not upstream LaTeX behavior).
pub const MAX_MATH_LAYOUT_CELLS: usize = 250_000;
/// Maximum cached render outputs (OC2 bound; upstream caches per block id).
pub const MAX_CACHE_ENTRIES: usize = 32;
/// Maximum retained last-good frames (OC2 bound).
pub const MAX_LAST_GOOD_BLOCKS: usize = 32;
/// Horizontal scrollbar is hidden (`packages/latex/src/markdown.ts:96`).
pub const HORIZONTAL_SCROLL_BAR_VISIBLE: bool = false;
/// Vertical scrollbar is hidden (`packages/latex/src/markdown.ts:97`).
pub const VERTICAL_SCROLL_BAR_VISIBLE: bool = false;
/// `scrollY: false` (`packages/latex/src/markdown.ts:88`).
pub const SCROLL_Y_ENABLED: bool = false;
/// Code-fence languages routed to the LaTeX renderer. `latex` and `math` are
/// registered by `packages/latex/src/plugin.ts:11-12`; `tex` is accepted
/// because upstream's Markdown test routes a `tex` fence through
/// `packages/latex/src/markdown.test.ts:53`.
pub const MATH_LANGUAGES: [&str; 3] = ["latex", "math", "tex"];

// ── Fence recognition and info-string parsing ─────────────────────────

/// Parsed opening fence line (same normalization as the Markdown flow's
/// `fence_parts`; the painted content is `token.text`,
/// `Markdown.ts:1171-1186`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MathFenceInfo {
    pub marker: char,
    pub marker_len: usize,
    pub info: String,
    pub language: Option<String>,
}

/// Parse the opening fence line of `raw`.
#[must_use]
pub fn parse_fence_info(raw: &str) -> Option<MathFenceInfo> {
    let first = raw.split('\n').next().unwrap_or("").trim_end_matches('\r');
    let indent = first.len() - first.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let trimmed = first.trim_start_matches(' ');
    let marker = *trimmed.as_bytes().first()? as char;
    if marker != '`' && marker != '~' {
        return None;
    }
    let marker_len = trimmed
        .as_bytes()
        .iter()
        .take_while(|byte| **byte == marker as u8)
        .count();
    if marker_len < 3 {
        return None;
    }
    let info = trimmed[marker_len..].trim().to_owned();
    let language = canonical_language(&info);
    Some(MathFenceInfo {
        marker,
        marker_len,
        info,
        language,
    })
}

fn canonical_language(info: &str) -> Option<String> {
    let token = info.split_whitespace().next()?;
    let cleaned = token.trim_matches(['{', '}', '.']).to_ascii_lowercase();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

/// True when `language` routes to the LaTeX/math renderer
/// (`packages/latex/src/plugin.ts:11-12` plus the `tex` alias at
/// `packages/latex/src/markdown.test.ts:53`).
#[must_use]
pub fn accepts_language(language: &str) -> bool {
    canonical_language(language)
        .is_some_and(|language| MATH_LANGUAGES.contains(&language.as_str()))
}

/// The closing-fence test used by upstream's streaming probe:
/// `\n {0,3}<marker>{marker_len,}\s*$` (`packages/latex/src/markdown.ts:38-42`).
#[must_use]
pub fn fence_closed_at_end(raw: &str, marker: char, marker_len: usize) -> bool {
    let trimmed = raw.trim_end();
    let Some(last_newline) = trimmed.rfind('\n') else {
        return false;
    };
    let line = &trimmed[last_newline + 1..];
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return false;
    }
    let rest = line.trim_start_matches(' ');
    let count = rest
        .as_bytes()
        .iter()
        .take_while(|byte| **byte == marker as u8)
        .count();
    count >= marker_len && rest[count..].is_empty()
}

// ── Limits ────────────────────────────────────────────────────────────

/// `assertSourceLength` (`packages/latex/src/limits.ts:14-18`).
///
/// # Errors
/// Returns the measured length when it exceeds `maximum`.
pub fn check_source_length(source: &str, maximum: usize) -> Result<usize, usize> {
    let chars = source.chars().count();
    if chars > maximum {
        Err(chars)
    } else {
        Ok(chars)
    }
}

/// `assertNestingDepth` (`packages/latex/src/limits.ts:20-40`).
///
/// Returns the maximum depth reached, or the char offset of the brace that
/// exceeded `maximum`. Escaped braces (`\{`, `\}`) are ignored only when the
/// backslash run has odd length; unmatched closing braces floor at zero.
///
/// # Errors
/// Returns `Err(offset)` for the first unescaped `{` that exceeds `maximum`.
pub fn check_nesting_depth(source: &str, maximum: usize) -> Result<usize, usize> {
    let mut depth = 0usize;
    let mut maximum_reached = 0usize;
    let mut slash_run = 0usize;
    for (offset, ch) in source.chars().enumerate() {
        if ch == '\\' {
            slash_run += 1;
            continue;
        }
        let escaped = slash_run % 2 == 1;
        slash_run = 0;
        if ch == '{' && !escaped {
            depth += 1;
            maximum_reached = maximum_reached.max(depth);
            if depth > maximum {
                return Err(offset);
            }
        } else if ch == '}' && !escaped {
            depth = depth.saturating_sub(1);
        }
    }
    Ok(maximum_reached)
}

// ── Fence value object ────────────────────────────────────────────────

/// One math fence handed to the renderer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MathFence {
    /// Stable Markdown block identity (`MarkdownBlock.id`; upstream
    /// `fallback?.id`, `packages/latex/src/markdown.ts:34`).
    pub block_id: usize,
    /// Canonical lowercased language token (`latex`, `math` or `tex`).
    pub language: String,
    /// `token.text`: fence content without the fence lines and info string.
    pub source: String,
    /// Opening fence marker and run length from `token.raw`, when present.
    pub marker: Option<(char, usize)>,
    /// True when `raw` ends with a matching closing fence.
    pub closed: bool,
    /// Flow `streaming` option.
    pub streaming: bool,
    /// Original source length in chars (may exceed `source.chars().count()`).
    pub total_chars: usize,
    /// True when `source` was clipped at [`MAX_MATH_SOURCE_CHARS`].
    pub truncated: bool,
}

impl MathFence {
    /// Build a fence from the flow's code-block fields. Returns `None` when
    /// the language is not routed to the LaTeX renderer.
    #[must_use]
    pub fn from_code_block(
        info_language: Option<&str>,
        source: &str,
        raw: &str,
        block_id: usize,
        streaming: bool,
    ) -> Option<Self> {
        let language = canonical_language(info_language?)?;
        if !MATH_LANGUAGES.contains(&language.as_str()) {
            return None;
        }
        let total_chars = source.chars().count();
        let (source, truncated) = clip_source(source);
        let parsed = parse_fence_info(raw);
        let marker = parsed.as_ref().map(|info| (info.marker, info.marker_len));
        let closed = marker.is_some_and(|(marker, marker_len)| {
            fence_closed_at_end(raw, marker, marker_len)
        });
        Some(Self {
            block_id,
            language,
            source,
            marker,
            closed,
            streaming,
            total_chars,
            truncated,
        })
    }

    /// Upstream streaming probe: the flow is streaming, an opening fence was
    /// seen and the closing fence has not arrived yet
    /// (`packages/latex/src/markdown.ts:37-42`).
    #[must_use]
    pub fn is_streaming(&self) -> bool {
        self.streaming && self.marker.is_some() && !self.closed
    }

    /// True when the fence is larger than the accepted bound.
    #[must_use]
    pub fn exceeds_bound(&self) -> bool {
        self.truncated
    }
}

fn clip_source(source: &str) -> (String, bool) {
    let mut end = 0;
    for (count, (offset, ch)) in source.char_indices().enumerate() {
        if count == MAX_MATH_SOURCE_CHARS {
            return (source[..offset].to_owned(), true);
        }
        end = offset + ch.len_utf8();
    }
    (source[..end].to_owned(), false)
}

// ── Theme key ─────────────────────────────────────────────────────────

/// Cache identity of the math palette. Upstream paints with
/// `theme.text.base` and `theme.text.muted`
/// (`packages/latex/src/plugin.ts:7-10`); this module keeps only those two
/// color strings as the cache key and does not resolve colors.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MathTheme {
    pub text: String,
    pub subdued: String,
}

impl MathTheme {
    #[must_use]
    pub fn new(text: impl Into<String>, subdued: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            subdued: subdued.into(),
        }
    }

    /// Stable cache identity.
    #[must_use]
    pub fn id(&self) -> String {
        format!("{}|{}", self.text, self.subdued)
    }
}

impl Default for MathTheme {
    fn default() -> Self {
        Self::new("#ffffff", "#808080")
    }
}

// ── Layout boundary ───────────────────────────────────────────────────

/// Upstream `MathStyle` (`packages/latex/src/types.ts:73-78`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MathStyle {
    pub color: Option<String>,
    pub bold: bool,
    pub italic: bool,
    pub dim: bool,
}

impl MathStyle {
    /// Shorthand used by variant layouts (`layout.ts` `withVariant`).
    #[must_use]
    pub fn flagged(bold: bool, italic: bool) -> Option<Self> {
        if bold || italic {
            Some(Self {
                bold,
                italic,
                ..Self::default()
            })
        } else {
            None
        }
    }
}

/// Upstream `MathCell` (`packages/latex/src/types.ts:80-83`): one glyph in
/// the laid-out formula grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MathCell {
    pub ch: char,
    pub style: Option<MathStyle>,
}

impl MathCell {
    #[must_use]
    pub const fn new(ch: char) -> Self {
        Self { ch, style: None }
    }

    #[must_use]
    pub const fn styled(ch: char, style: MathStyle) -> Self {
        Self {
            ch,
            style: Some(style),
        }
    }
}

/// Projection of upstream `MathLayout`
/// (`packages/latex/src/types.ts:85-91`): the cell grid plus the geometry
/// the scroll box and transcript footprint need.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MathLayout {
    pub width: usize,
    pub height: usize,
    pub baseline: usize,
    /// `layout.cells`: `height` rows of `width` optional styled glyphs.
    pub cells: Vec<Vec<Option<MathCell>>>,
}

impl MathLayout {
    /// Geometry with an all-blank grid (stub/sentinel layouts).
    #[must_use]
    pub fn new(width: usize, height: usize, baseline: usize) -> Self {
        let cells = vec![vec![None; width]; height.max(1)];
        Self {
            width,
            height,
            baseline,
            cells,
        }
    }

    /// Real layout: takes ownership of a `height x width` grid built by the
    /// layout engine (`layout.ts` `blank`).
    #[must_use]
    pub fn with_cells(
        width: usize,
        height: usize,
        baseline: usize,
        cells: Vec<Vec<Option<MathCell>>>,
    ) -> Self {
        debug_assert_eq!(cells.len(), height.max(1), "cell rows must match height");
        debug_assert!(
            cells.iter().all(|row| row.len() == width),
            "cell rows must match width"
        );
        Self {
            width,
            height,
            baseline,
            cells,
        }
    }

    /// Row text the way upstream `MathLayout.toString()` renders it
    /// (`types.ts:96-107`): blanks for absent cells, wide glyphs skip their
    /// following column, trailing whitespace trimmed.
    #[must_use]
    pub fn row_text(&self, y: usize) -> String {
        let mut output = String::new();
        let mut x = 0usize;
        while x < self.width {
            let cell = self
                .cells
                .get(y)
                .and_then(|row| row.get(x))
                .and_then(|cell| cell.as_ref());
            if let Some(cell) = cell {
                output.push(cell.ch);
            } else {
                output.push(' ');
            }
            // Upstream `toString` (`layout.ts:656-666`) always advances at
            // least one column; a zero-width glyph (combining mark) must not
            // stall the scan.
            x += cell_width(output.chars().last().unwrap_or(' ')).max(1);
        }
        output.trim_end().to_owned()
    }
}

/// Upstream `cellWidth` (`packages/latex/src/layout.ts:617-645`): zero-width
/// for control, combining and variation-selector runes, wide for CJK/emoji
/// runs, one column otherwise. Bold-free mirror of the same table.
#[must_use]
pub fn cell_width(ch: char) -> usize {
    let code = ch as u32;
    if matches!(code, 0x0000..=0x001f | 0x007f..=0x009f | 0x0300..=0x036f | 0xfe00..=0xfe0f) {
        return 0;
    }
    if code >= 0x1100
        && (code <= 0x115f
            || code == 0x2329
            || code == 0x232a
            || (0x2e80..=0xa4cf).contains(&code)
            || (0xac00..=0xd7a3).contains(&code)
            || (0xf900..=0xfaff).contains(&code)
            || (0xfe10..=0xfe6f).contains(&code)
            || (0xff00..=0xff60).contains(&code)
            || (0xffe0..=0xffe6).contains(&code)
            || (0x1f300..=0x1faff).contains(&code))
    {
        return 2;
    }
    1
}

/// Layout request. `display_mode` is always true, matching
/// `packages/latex/src/markdown.ts:114`.
#[derive(Clone, Copy, Debug)]
pub struct MathLayoutRequest<'a> {
    pub source: &'a str,
    pub display_mode: bool,
    pub theme: &'a MathTheme,
}

/// Layout failure. Mirrors `LatexParseError`/`RangeError`
/// (`packages/latex/src/types.ts:63-71`, `markdown.ts:112-118`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MathFailure {
    /// Invalid or unsupported syntax; `offset` is upstream's error position.
    Parse { offset: usize },
    /// Source-length limit.
    TooLarge { chars: usize },
    /// Brace-nesting limit.
    TooDeep { offset: usize },
    /// No engine configured.
    Unavailable,
}

/// LaTeX layout boundary. Implementations own parsing and layout.
pub trait MathLayoutEngine {
    fn layout(&self, request: &MathLayoutRequest<'_>) -> Result<MathLayout, MathFailure>;
}

/// Fail-closed engine: every formula keeps its plain code block.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoMathEngine;

impl MathLayoutEngine for NoMathEngine {
    fn layout(&self, _request: &MathLayoutRequest<'_>) -> Result<MathLayout, MathFailure> {
        Err(MathFailure::Unavailable)
    }
}

// ── Outcome ───────────────────────────────────────────────────────────

/// Rendered formula footprint. `source` is the exact source the layout was
/// produced from, which is the *previous* source for a retained frame
/// (`packages/latex/src/markdown.ts:44-48`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MathFrame {
    pub source: String,
    pub layout: MathLayout,
    /// Always [`MATH_MARGIN_TOP`] (`packages/latex/src/markdown.ts:86`).
    pub margin_top: usize,
    /// True when the previous good layout was reused because the new source
    /// extends it and the fence is still streaming
    /// (`packages/latex/src/markdown.ts:36,44-48`).
    pub retained: bool,
    /// True when the frame came from the `(source, width, theme)` cache.
    pub from_cache: bool,
}

impl MathFrame {
    /// Rows of the formula viewport (`height: layout.height`).
    #[must_use]
    pub fn rows(&self) -> usize {
        self.layout.height
    }

    /// Rows reserved in the transcript, including the scroll-box margin.
    #[must_use]
    pub fn footprint_rows(&self) -> usize {
        self.margin_top + self.layout.height
    }

    /// Content width: `width: "100%"` with `minWidth: layout.width`
    /// (`packages/latex/src/markdown.ts:75-77`).
    #[must_use]
    pub fn content_width(&self, container_width: usize) -> usize {
        self.layout.width.max(container_width)
    }

    /// Footprint cells for a container of `container_width` columns.
    #[must_use]
    pub fn footprint_cells(&self, container_width: usize) -> usize {
        self.content_width(container_width)
            .saturating_mul(self.footprint_rows())
    }

    /// Horizontal scroll box for this formula.
    #[must_use]
    pub fn scroll_box(&self, container_width: usize) -> MathScrollBox {
        MathScrollBox::new(self.layout.width, container_width)
    }
}

/// Why the fence stays a plain code block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MathFallbackReason {
    /// `LatexParseError`: incomplete, invalid or unsupported syntax.
    UnsupportedSyntax { offset: usize },
    /// `RangeError`: source length (`limits.ts:14-18`).
    SourceTooLarge { chars: usize },
    /// `RangeError`: nesting depth (`limits.ts:20-40`).
    TooDeep { offset: usize },
    /// Upstream would draw a zero-height or impossible layout; OC2 refuses.
    InvalidLayout,
    /// Layout exceeds the OC2 cell bound.
    OversizedLayout { width: usize, height: usize },
    /// No engine configured.
    RendererUnavailable,
}

impl fmt::Display for MathFallbackReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSyntax { offset } => {
                write!(f, "unsupported LaTeX syntax at offset {offset}")
            }
            Self::SourceTooLarge { chars } => {
                write!(f, "LaTeX source of {chars} characters exceeds the limit")
            }
            Self::TooDeep { offset } => {
                write!(f, "LaTeX nesting limit exceeded at offset {offset}")
            }
            Self::InvalidLayout => f.write_str("LaTeX layout is empty or invalid"),
            Self::OversizedLayout { width, height } => {
                write!(f, "LaTeX layout {width}x{height} exceeds the cell limit")
            }
            Self::RendererUnavailable => f.write_str("no LaTeX renderer configured"),
        }
    }
}

/// Plain code-block fallback; the exact source is preserved
/// (`packages/latex/src/markdown.test.ts:73-84`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MathFallback {
    pub reason: MathFallbackReason,
    pub source: String,
}

impl MathFallback {
    /// Rows the fallback code block occupies (one per source line, no math
    /// margin; `Markdown.ts:1171-1186`).
    #[must_use]
    pub fn rows(&self) -> usize {
        self.source.split('\n').count()
    }
}

/// Renderer decision for one fence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MathOutcome {
    Rendered(MathFrame),
    Fallback(MathFallback),
}

impl MathOutcome {
    #[must_use]
    pub fn frame(&self) -> Option<&MathFrame> {
        match self {
            Self::Rendered(frame) => Some(frame),
            Self::Fallback(_) => None,
        }
    }

    #[must_use]
    pub fn fallback(&self) -> Option<&MathFallback> {
        match self {
            Self::Rendered(_) => None,
            Self::Fallback(fallback) => Some(fallback),
        }
    }
}

// ── Render cache ──────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CacheKey {
    source: String,
    width: usize,
    theme: String,
}

#[derive(Debug, Default)]
struct RenderCache {
    entries: BTreeMap<CacheKey, MathLayout>,
    order: VecDeque<CacheKey>,
}

impl RenderCache {
    fn get(&self, key: &CacheKey) -> Option<MathLayout> {
        self.entries.get(key).cloned()
    }

    fn insert(&mut self, key: CacheKey, value: MathLayout) {
        if self.entries.insert(key.clone(), value).is_none() {
            self.order.push_back(key);
        }
        while self.entries.len() > MAX_CACHE_ENTRIES {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.entries.remove(&oldest);
        }
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }
}

#[derive(Clone, Debug)]
struct LastGood {
    source: String,
    layout: MathLayout,
}

// ── Renderer ──────────────────────────────────────────────────────────

/// Fence renderer following `packages/latex/src/markdown.ts:27-51`: attempt a
/// layout, retain the previous layout only when the source extends it and the
/// fence is streaming, otherwise leave the plain code block.
#[derive(Debug)]
pub struct MathRenderer<E: MathLayoutEngine> {
    engine: E,
    theme: MathTheme,
    cache: RenderCache,
    last_good: BTreeMap<usize, LastGood>,
    last_good_order: Vec<usize>,
}

impl MathRenderer<NoMathEngine> {
    /// Fail-closed renderer: everything falls back to the code block.
    #[must_use]
    pub fn without_engine(theme: MathTheme) -> Self {
        Self::new(NoMathEngine, theme)
    }
}

impl<E: MathLayoutEngine> MathRenderer<E> {
    #[must_use]
    pub fn new(engine: E, theme: MathTheme) -> Self {
        Self {
            engine,
            theme,
            cache: RenderCache::default(),
            last_good: BTreeMap::new(),
            last_good_order: Vec::new(),
        }
    }

    #[must_use]
    pub fn theme(&self) -> &MathTheme {
        &self.theme
    }

    /// Theme colors are part of the cache key (`plugin.ts:7-10`).
    pub fn set_theme(&mut self, theme: MathTheme) {
        self.theme = theme;
    }

    #[must_use]
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    #[must_use]
    pub fn last_good_source(&self, block_id: usize) -> Option<&str> {
        self.last_good
            .get(&block_id)
            .map(|entry| entry.source.as_str())
    }

    /// Drop retained state for a destroyed block
    /// (`packages/latex/src/markdown.ts:99-108`).
    pub fn forget_block(&mut self, block_id: usize) {
        if self.last_good.remove(&block_id).is_some() {
            self.last_good_order.retain(|id| *id != block_id);
        }
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Resolve one fence.
    pub fn render(&mut self, fence: &MathFence, container_width: usize) -> MathOutcome {
        let fallback = |reason: MathFallbackReason| {
            MathOutcome::Fallback(MathFallback {
                reason,
                source: fence.source.clone(),
            })
        };

        if fence.truncated {
            // OC2 bound: refuse instead of laying out a clipped formula.
            return fallback(MathFallbackReason::SourceTooLarge {
                chars: fence.total_chars,
            });
        }

        // `markdown.ts:34-36`: retention requires the new source to extend the
        // previous source. Computed before the layout attempt, exactly like
        // upstream.
        let retained = self
            .last_good
            .get(&fence.block_id)
            .filter(|previous| fence.source.starts_with(&previous.source))
            .cloned();

        let key = CacheKey {
            source: fence.source.clone(),
            width: container_width,
            theme: self.theme.id(),
        };
        if let Some(layout) = self.cache.get(&key) {
            self.claim(fence.block_id, fence.source.clone(), &layout);
            return MathOutcome::Rendered(MathFrame {
                source: fence.source.clone(),
                layout,
                margin_top: MATH_MARGIN_TOP,
                retained: false,
                from_cache: true,
            });
        }

        let failure = match check_source_length(&fence.source, MAX_MATH_SOURCE_CHARS) {
            Err(chars) => Some(MathFallbackReason::SourceTooLarge { chars }),
            Ok(_) => match check_nesting_depth(&fence.source, MAX_MATH_NESTING_DEPTH) {
                Err(offset) => Some(MathFallbackReason::TooDeep { offset }),
                Ok(_) => None,
            },
        };

        let layout = match failure {
            Some(reason) => Err(reason),
            None => {
                let result = {
                    let request = MathLayoutRequest {
                        source: &fence.source,
                        display_mode: true,
                        theme: &self.theme,
                    };
                    self.engine.layout(&request)
                };
                match result {
                    Ok(layout) => validate_layout(layout),
                    Err(MathFailure::Parse { offset }) => {
                        Err(MathFallbackReason::UnsupportedSyntax { offset })
                    }
                    Err(MathFailure::TooLarge { chars }) => {
                        Err(MathFallbackReason::SourceTooLarge { chars })
                    }
                    Err(MathFailure::TooDeep { offset }) => {
                        Err(MathFallbackReason::TooDeep { offset })
                    }
                    Err(MathFailure::Unavailable) => {
                        Err(MathFallbackReason::RendererUnavailable)
                    }
                }
            }
        };

        match layout {
            Ok(layout) => {
                self.store(key, fence.block_id, &fence.source, &layout);
                MathOutcome::Rendered(MathFrame {
                    source: fence.source.clone(),
                    layout,
                    margin_top: MATH_MARGIN_TOP,
                    retained: false,
                    from_cache: false,
                })
            }
            Err(reason) => {
                if fence.is_streaming() {
                    if let Some(previous) = retained {
                        // `markdown.ts:44-48`: keep the last good frame.
                        self.claim(fence.block_id, previous.source.clone(), &previous.layout);
                        return MathOutcome::Rendered(MathFrame {
                            source: previous.source,
                            layout: previous.layout,
                            margin_top: MATH_MARGIN_TOP,
                            retained: true,
                            from_cache: false,
                        });
                    }
                }
                fallback(reason)
            }
        }
    }

    fn store(&mut self, key: CacheKey, block_id: usize, source: &str, layout: &MathLayout) {
        self.cache.insert(key, layout.clone());
        self.claim(block_id, source.to_owned(), layout);
    }

    /// `lastGood.set(key, frame)` (`markdown.ts:100-104`), bounded per block.
    fn claim(&mut self, block_id: usize, source: String, layout: &MathLayout) {
        if !self.last_good.contains_key(&block_id) {
            self.last_good_order.push(block_id);
            while self.last_good_order.len() > MAX_LAST_GOOD_BLOCKS {
                let oldest = self.last_good_order.remove(0);
                self.last_good.remove(&oldest);
            }
        }
        self.last_good
            .insert(block_id, LastGood { source, layout: layout.clone() });
    }
}

fn validate_layout(layout: MathLayout) -> Result<MathLayout, MathFallbackReason> {
    if layout.height == 0 || layout.width == 0 {
        return Err(MathFallbackReason::InvalidLayout);
    }
    if layout.baseline > layout.height {
        return Err(MathFallbackReason::InvalidLayout);
    }
    if layout.width.saturating_mul(layout.height) > MAX_MATH_LAYOUT_CELLS {
        return Err(MathFallbackReason::OversizedLayout {
            width: layout.width,
            height: layout.height,
        });
    }
    Ok(layout)
}

// ── Horizontal scroll box ─────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Horizontal-only scroll box (`packages/latex/src/markdown.ts:80-98`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MathScrollBox {
    content_width: usize,
    viewport_width: usize,
    scroll_x: usize,
}

impl MathScrollBox {
    #[must_use]
    pub fn new(content_width: usize, viewport_width: usize) -> Self {
        Self {
            content_width,
            viewport_width,
            scroll_x: 0,
        }
    }

    #[must_use]
    pub fn scroll_x(&self) -> usize {
        self.scroll_x
    }

    #[must_use]
    pub fn max_scroll_x(&self) -> usize {
        self.content_width.saturating_sub(self.viewport_width)
    }

    /// Apply a horizontal scroll delta (clamped; the base ScrollBox owns the
    /// actual delta, the upstream override only filters events).
    pub fn scroll_by(&mut self, delta: isize) {
        let next = if delta >= 0 {
            self.scroll_x.saturating_add(delta.unsigned_abs())
        } else {
            self.scroll_x.saturating_sub(delta.unsigned_abs())
        };
        self.scroll_x = next.min(self.max_scroll_x());
    }

    /// `onMouseScroll` (`packages/latex/src/markdown.ts:89-95`): shift-modified
    /// or horizontal wheel events are consumed; plain vertical wheel events
    /// pass through so the transcript keeps scrolling.
    pub fn on_mouse_scroll(&mut self, direction: ScrollDirection, shift: bool) -> bool {
        shift || matches!(direction, ScrollDirection::Left | ScrollDirection::Right)
    }

    /// `viewport.horizontalScrollBar.visible = false` (`markdown.ts:96`).
    #[must_use]
    pub fn horizontal_scroll_bar_visible(&self) -> bool {
        HORIZONTAL_SCROLL_BAR_VISIBLE
    }

    /// `viewport.verticalScrollBar.visible = false` (`markdown.ts:97`).
    #[must_use]
    pub fn vertical_scroll_bar_visible(&self) -> bool {
        VERTICAL_SCROLL_BAR_VISIBLE
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct StubEngine {
        responses: RefCell<Vec<Result<MathLayout, MathFailure>>>,
        calls: RefCell<Vec<(String, usize)>>, // (source, char count)
        default: MathLayout,
    }

    impl StubEngine {
        fn new(responses: Vec<Result<MathLayout, MathFailure>>) -> Self {
            Self {
                responses: RefCell::new(responses),
                calls: RefCell::new(Vec::new()),
                default: MathLayout::new(5, 3, 2),
            }
        }

        fn call_count(&self) -> usize {
            self.calls.borrow().len()
        }
    }

    impl MathLayoutEngine for StubEngine {
        fn layout(&self, request: &MathLayoutRequest<'_>) -> Result<MathLayout, MathFailure> {
            self.calls
                .borrow_mut()
                .push((request.source.to_owned(), request.source.chars().count()));
            assert!(request.display_mode, "displayMode: true (markdown.ts:114)");
            let mut responses = self.responses.borrow_mut();
            if responses.is_empty() {
                return Ok(self.default.clone());
            }
            responses.remove(0)
        }
    }

    fn fence(source: &str, raw: &str, block_id: usize, streaming: bool) -> MathFence {
        MathFence::from_code_block(Some("latex"), source, raw, block_id, streaming)
            .expect("latex fence")
    }

    fn renderer(engine: StubEngine) -> MathRenderer<StubEngine> {
        MathRenderer::new(engine, MathTheme::new("#abcdef", "#667788"))
    }

    /// MD-009. Upstream: `packages/latex/src/plugin.ts:11-12` (languages
    /// `latex` and `math`), `packages/latex/src/markdown.test.ts:53` (`tex`
    /// and `LATEX title=example` reach the renderer) and
    /// `packages/latex/src/markdown.ts:37-42` (streaming requires an unclosed
    /// fence).
    #[test]
    fn md_009_math_fence_languages_and_info_string() {
        for language in ["latex", "LATEX", "LaTeX", "math", "tex", "MATH"] {
            assert!(accepts_language(language), "{language}");
            assert!(
                MathFence::from_code_block(Some(language), "x", "```latex\nx\n```\n", 0, false)
                    .is_some(),
                "{language}"
            );
        }
        for language in ["ts", "text", "mermaid", "latex2", ""] {
            assert!(!accepts_language(language), "{language}");
            assert!(
                MathFence::from_code_block(Some(language), "x", "```ts\nx\n```\n", 0, false)
                    .is_none(),
                "{language}"
            );
        }
        assert!(MathFence::from_code_block(None, "x", "```\nx\n```\n", 0, false).is_none());

        let info = parse_fence_info("```LATEX title=example\n").expect("fence");
        assert_eq!(info.marker, '`');
        assert_eq!(info.marker_len, 3);
        assert_eq!(info.language.as_deref(), Some("latex"));
        assert_eq!(info.info, "LATEX title=example");
        assert!(parse_fence_info("    ```latex\n").is_none());
        assert!(parse_fence_info("``latex\n").is_none());
        assert!(parse_fence_info("text\n").is_none());

        // Streaming: unclosed fence while the flow is streaming.
        let open = fence("\\frac{1}{", "```latex\n\\frac{1}{", 1, true);
        assert!(open.is_streaming());
        assert!(!fence("\\frac{1}{2}", "```latex\n\\frac{1}{2}\n```\n", 1, true).is_streaming());
        assert!(!fence("x", "~~~latex\nx\n~~~\n", 1, true).is_streaming());
        assert!(!fence("x", "```latex\nx", 1, false).is_streaming());

        // Fence marker variants.
        let tilde = parse_fence_info("~~~math\n").expect("tilde");
        assert_eq!(tilde.marker, '~');
        assert_eq!(tilde.marker_len, 3);
        assert!(fence_closed_at_end("~~~math\nx\n~~~~\n", '~', 3));
        assert!(!fence_closed_at_end("~~~math\nx\n~~\n", '~', 3));
        assert!(!fence_closed_at_end("~~~math\nx", '~', 3));
    }

    /// MD-009. Upstream: `packages/latex/src/markdown.ts:34-48` – the previous
    /// layout is retained only when the new source starts with the previous
    /// source **and** the fence is still streaming; otherwise the block falls
    /// back to its source.
    #[test]
    fn md_009_retains_last_good_only_when_source_extends_it() {
        let engine = StubEngine::new(vec![
            Ok(MathLayout::new(5, 3, 2)),                       // initial formula
            Err(MathFailure::Parse { offset: 9 }),              // extended, incomplete
            Err(MathFailure::Parse { offset: 0 }),              // replaced content
            Err(MathFailure::Parse { offset: 0 }),              // extension but not streaming
        ]);
        let mut renderer = renderer(engine);

        let first = renderer.render(&fence("\\frac{1}{2}", "```latex\n\\frac{1}{2}\n```\n", 4, true), 80);
        let frame = first.frame().expect("initial layout");
        assert_eq!(frame.layout, MathLayout::new(5, 3, 2));
        assert!(!frame.retained);
        assert_eq!(renderer.last_good_source(4), Some("\\frac{1}{2}"));

        // The new source extends the previous one and the fence is open.
        let extended = "\\frac{1}{2} + ";
        let second = renderer.render(&fence(extended, "```latex\n\\frac{1}{2} + ", 4, true), 80);
        let retained = second.frame().expect("retained layout");
        assert!(retained.retained);
        assert_eq!(retained.source, "\\frac{1}{2}");
        assert_eq!(retained.layout, MathLayout::new(5, 3, 2));

        // A different source does not extend the previous one.
        let replaced = renderer.render(&fence("x^2", "```latex\nx^2", 4, true), 80);
        let fallback = replaced.fallback().expect("replaced source falls back");
        assert_eq!(
            fallback.reason,
            MathFallbackReason::UnsupportedSyntax { offset: 0 }
        );
        assert_eq!(fallback.source, "x^2");

        // Extension retention requires streaming (closed fence -> fallback).
        let closed = renderer.render(
            &fence("\\frac{1}{2}y", "```latex\n\\frac{1}{2}y\n```\n", 4, true),
            80,
        );
        assert!(closed.fallback().is_some());
    }

    /// MD-009. Upstream: `packages/latex/src/markdown.test.ts:73-84` and
    /// `packages/latex/src/markdown.ts:44-50` – unsupported syntax keeps the
    /// exact source as a code block.
    #[test]
    fn md_009_unsupported_syntax_falls_back_with_source() {
        let engine = StubEngine::new(vec![Err(MathFailure::Parse { offset: 3 })]);
        let mut renderer = renderer(engine);
        let source = "\\unsupported{x}";
        let outcome = renderer.render(
            &fence(source, "```latex\n\\unsupported{x}\n```\n", 2, false),
            80,
        );
        let fallback = outcome.fallback().expect("fallback");
        assert_eq!(
            fallback.reason,
            MathFallbackReason::UnsupportedSyntax { offset: 3 }
        );
        assert_eq!(fallback.source, source);
        assert_eq!(fallback.rows(), 1);
        // Nothing was retained for a failed non-streaming fence.
        assert_eq!(renderer.last_good_source(2), None);
    }

    /// MD-009. Upstream: `packages/latex/src/limits.ts:3-4,14-40` and
    /// `packages/latex/src/parser.ts:65-77`.
    #[test]
    fn md_009_limits_follow_upstream() {
        assert_eq!(check_source_length("abc", 3), Ok(3));
        assert_eq!(check_source_length("abcd", 3), Err(4));

        assert_eq!(check_nesting_depth("{{}}", 2), Ok(2));
        assert_eq!(check_nesting_depth("{{{", 2), Err(2));
        // Escaped braces are not structural; an odd backslash run escapes.
        assert_eq!(check_nesting_depth("\\{", 0), Ok(0));
        assert_eq!(check_nesting_depth("\\\\{", 0), Err(2));
        // Unmatched closing braces floor at zero.
        assert_eq!(check_nesting_depth("}{", 1), Ok(1));

        let nested = format!("{}{}", "{".repeat(MAX_MATH_NESTING_DEPTH), "}".repeat(MAX_MATH_NESTING_DEPTH));
        assert!(check_nesting_depth(&nested, MAX_MATH_NESTING_DEPTH).is_ok());
        let too_deep = "{".repeat(MAX_MATH_NESTING_DEPTH + 1);
        assert_eq!(
            check_nesting_depth(&too_deep, MAX_MATH_NESTING_DEPTH),
            Err(MAX_MATH_NESTING_DEPTH)
        );

        let rejected = StubEngine::new(vec![]);
        let mut renderer = renderer(rejected);
        let outcome = renderer.render(
            &fence(&too_deep, &format!("```latex\n{too_deep}\n```\n"), 1, false),
            80,
        );
        assert_eq!(
            outcome.fallback().map(|fallback| fallback.reason),
            Some(MathFallbackReason::TooDeep {
                offset: MAX_MATH_NESTING_DEPTH
            })
        );
        assert_eq!(renderer.engine.call_count(), 0);

        // A fence over the source bound is clipped at construction and fails
        // closed without reaching the engine.
        let huge = "x".repeat(MAX_MATH_SOURCE_CHARS + 1);
        let oversized = fence(&huge, &format!("```latex\n{huge}\n```\n"), 2, false);
        assert!(oversized.truncated);
        let outcome = renderer.render(&oversized, 80);
        assert_eq!(
            outcome.fallback().map(|fallback| fallback.reason),
            Some(MathFallbackReason::SourceTooLarge {
                chars: MAX_MATH_SOURCE_CHARS + 1
            })
        );
        assert_eq!(check_source_length(&huge, MAX_MATH_SOURCE_CHARS).unwrap_err(), MAX_MATH_SOURCE_CHARS + 1);
        assert_eq!(renderer.engine.call_count(), 0);
    }

    /// MD-009. Upstream: `packages/latex/src/markdown.ts:80-98` – the formula
    /// lives in a horizontal-only scroll box; plain vertical wheel events are
    /// not consumed, and both scrollbars are hidden.
    #[test]
    fn md_009_long_formula_scrolls_horizontally_without_stealing_vertical() {
        let frame = MathFrame {
            source: "\\frac{1}{2}".to_owned(),
            layout: MathLayout::new(300, 2, 1),
            margin_top: MATH_MARGIN_TOP,
            retained: false,
            from_cache: false,
        };
        assert_eq!(frame.rows(), 2);
        assert_eq!(frame.footprint_rows(), 3);
        assert_eq!(frame.content_width(80), 300);
        assert_eq!(frame.content_width(400), 400);
        assert_eq!(frame.footprint_cells(80), 900);

        let mut scroll = frame.scroll_box(80);
        assert_eq!(scroll.max_scroll_x(), 220);
        scroll.scroll_by(500);
        assert_eq!(scroll.scroll_x(), 220);
        scroll.scroll_by(-300);
        assert_eq!(scroll.scroll_x(), 0);

        // Plain vertical wheel passes through to the transcript.
        assert!(!scroll.on_mouse_scroll(ScrollDirection::Up, false));
        assert!(!scroll.on_mouse_scroll(ScrollDirection::Down, false));
        // Shift-modified and horizontal wheel events are consumed.
        assert!(scroll.on_mouse_scroll(ScrollDirection::Up, true));
        assert!(scroll.on_mouse_scroll(ScrollDirection::Down, true));
        assert!(scroll.on_mouse_scroll(ScrollDirection::Left, false));
        assert!(scroll.on_mouse_scroll(ScrollDirection::Right, false));

        assert!(!scroll.horizontal_scroll_bar_visible());
        assert!(!scroll.vertical_scroll_bar_visible());
        assert!(!SCROLL_Y_ENABLED);
    }

    /// MD-009. OC2 cache key is `(source, width, theme)`; upstream caches the
    /// last good frame per block id (`packages/latex/src/markdown.ts:31-35`).
    #[test]
    fn md_009_render_cache_is_keyed_by_source_width_and_theme() {
        let engine = StubEngine::new(vec![]);
        let mut renderer = renderer(engine);
        let f = fence("\\frac{1}{2}", "```latex\n\\frac{1}{2}\n```\n", 1, false);

        assert!(matches!(
            renderer.render(&f, 40),
            MathOutcome::Rendered(ref frame) if !frame.from_cache
        ));
        assert_eq!(renderer.cache_len(), 1);

        assert!(matches!(
            renderer.render(&f, 40),
            MathOutcome::Rendered(ref frame) if frame.from_cache
        ));
        assert_eq!(renderer.engine.call_count(), 1);

        assert!(matches!(
            renderer.render(&f, 60),
            MathOutcome::Rendered(ref frame) if !frame.from_cache
        ));
        assert_eq!(renderer.engine.call_count(), 2);
        assert_eq!(renderer.cache_len(), 2);

        renderer.set_theme(MathTheme::new("#111111", "#222222"));
        assert!(matches!(
            renderer.render(&f, 40),
            MathOutcome::Rendered(ref frame) if !frame.from_cache
        ));
        assert_eq!(renderer.cache_len(), 3);
    }

    /// MD-009. No engine configured -> fail closed to the code block
    /// (`packages/latex/src/markdown.ts:44-50`).
    #[test]
    fn md_009_renderer_without_engine_falls_back() {
        let mut renderer = MathRenderer::without_engine(MathTheme::default());
        let outcome = renderer.render(
            &fence("\\frac{1}{2}", "```latex\n\\frac{1}{2}\n```\n", 1, false),
            80,
        );
        let fallback = outcome.fallback().expect("fallback");
        assert_eq!(fallback.reason, MathFallbackReason::RendererUnavailable);
        assert_eq!(fallback.source, "\\frac{1}{2}");
        assert_eq!(renderer.cache_len(), 0);
    }

    /// MD-009. Invalid/oversized layouts are refused (OC2 fail-closed bound)
    /// instead of reserving impossible geometry.
    #[test]
    fn md_009_invalid_layouts_fall_back() {
        let engine = StubEngine::new(vec![
            Ok(MathLayout::new(5, 0, 0)),
            Ok(MathLayout::new(500_000, 1, 0)),
            Ok(MathLayout::new(0, 3, 0)),
        ]);
        let mut renderer = renderer(engine);
        let f = fence("x", "```latex\nx\n```\n", 1, false);
        assert_eq!(
            renderer.render(&f, 80).fallback().map(|fallback| fallback.reason),
            Some(MathFallbackReason::InvalidLayout)
        );
        assert_eq!(
            renderer.render(&f, 80).fallback().map(|fallback| fallback.reason),
            Some(MathFallbackReason::OversizedLayout {
                width: 500_000,
                height: 1
            })
        );
        assert_eq!(
            renderer.render(&f, 80).fallback().map(|fallback| fallback.reason),
            Some(MathFallbackReason::InvalidLayout)
        );
    }
}
