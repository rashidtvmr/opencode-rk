//! Text/edit buffer FFI surface (pure-Rust bridge layer).
//!
//! Implements Rust-side models that mirror the upstream TypeScript/native
//! shapes found in:
//! - `packages/core/src/text-buffer.ts` (`TextBuffer`, `setText`,
//!   `setStyledText`, `addHighlight`/`Highlight`)
//! - `packages/core/src/lib/styled-text.ts` (`StyledText`, `TextChunk`)
//! - `packages/core/src/types.ts` (`Highlight`, `TextAttributes`,
//!   `MeasureResult`)
//! - `packages/native/src/text-buffer.zig` (`StyledChunk`)
//! - `packages/native/src/text-buffer-view.zig` (`MeasureResult`,
//!   `measureForDimensions`, wrap modes)
//!
//! This module is standalone-compilable (no `crate::` imports) so it can be
//! unit-tested in isolation via `rustc --test`. It is `forbid(unsafe_code)`
//! and bounded by [`MAX_TEXT_BYTES`]. Display-width (not char-count) is used
//! for clipping/wrapping/measuring, per the upstream intent noted in
//! `safe_renderer.rs::render_once` ("char-count clip ... upgrade: unicode-width").

#![forbid(unsafe_code)]

/// Byte cap for a single text payload. Mirrors
/// `crates/opentui-bridge/src/safe_renderer.rs:30` (`MAX_TEXT_BYTES`).
pub const MAX_TEXT_BYTES: usize = 64 * 1024;

/// Fail-closed error set mirroring `safe_renderer::BridgeError`'s relevant
/// variants for this module's bounded operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeError {
    AlreadyLive,
    CreateFailed,
    InvalidHandle,
    RenderFailed,
    ZeroSize,
    TextTooLarge,
    TitleNul,
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::AlreadyLive => "renderer already live",
            Self::CreateFailed => "renderer creation failed",
            Self::InvalidHandle => "invalid renderer handle",
            Self::RenderFailed => "render failed",
            Self::ZeroSize => "zero-size renderer",
            Self::TextTooLarge => "text exceeds size limit",
            Self::TitleNul => "title contains NUL byte",
        };
        f.write_str(s)
    }
}

impl std::error::Error for BridgeError {}

/// Validate `text` fits within [`MAX_TEXT_BYTES`] (byte length).
pub fn check_text_bytes(text: &str) -> Result<(), BridgeError> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(BridgeError::TextTooLarge);
    }
    Ok(())
}

/// TS `TextAttributes` bitflags (`packages/core/src/types.ts:8-18`).
pub mod attr {
    pub const NONE: u32 = 0;
    pub const BOLD: u32 = 1 << 0;
    pub const DIM: u32 = 1 << 1;
    pub const ITALIC: u32 = 1 << 2;
    pub const UNDERLINE: u32 = 1 << 3;
    pub const BLINK: u32 = 1 << 4;
    pub const INVERSE: u32 = 1 << 5;
    pub const HIDDEN: u32 = 1 << 6;
    pub const STRIKETHROUGH: u32 = 1 << 7;
}

/// Packed RGBA lane order: 4 `u16`, color byte in low 8 bits, one meta byte
/// per lane in high 8 bits (mirrors `color.rs::pack_rgba8` and Zig
/// `ansi.RGBA = [4]u16`).
pub type RgbaU16 = [u16; 4];

/// Simple unpacked RGBA (mirrors `color.rs::Rgba`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
    #[must_use]
    pub const fn transparent() -> Self {
        Self { r: 0, g: 0, b: 0, a: 0 }
    }
    #[must_use]
    pub const fn to_packed(self) -> RgbaU16 {
        [self.r as u16, self.g as u16, self.b as u16, self.a as u16]
    }
    #[must_use]
    pub const fn from_packed(lanes: RgbaU16) -> Self {
        Self {
            r: (lanes[0] & 0xff) as u8,
            g: (lanes[1] & 0xff) as u8,
            b: (lanes[2] & 0xff) as u8,
            a: (lanes[3] & 0xff) as u8,
        }
    }
}

/// A hyperlink attached to a chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub url: String,
}

/// A styled text chunk (`packages/core/src/lib/styled-text.ts` `TextChunk`,
/// Zig `text-buffer.zig:43` `StyledChunk`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextChunk {
    pub text: String,
    pub fg: Option<Rgba>,
    pub bg: Option<Rgba>,
    pub attributes: u32,
    pub link: Option<Link>,
}

impl TextChunk {
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            fg: None,
            bg: None,
            attributes: 0,
            link: None,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.text.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Display width of the chunk's text (cell columns), not byte length.
    #[must_use]
    pub fn display_width(&self) -> usize {
        display_width(&self.text)
    }
}

/// A styled text document made of ordered chunks (`StyledText`).
pub struct StyledText {
    pub chunks: Vec<TextChunk>,
}

impl StyledText {
    #[must_use]
    pub const fn new(chunks: Vec<TextChunk>) -> Self {
        Self { chunks }
    }

    #[must_use]
    pub fn from_string(s: &str) -> Self {
        Self {
            chunks: vec![TextChunk::new(s)],
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chunks.iter().all(|c| c.text.is_empty())
    }

    /// Concatenate all chunk text into a single owned string.
    #[must_use]
    pub fn to_plain_text(&self) -> String {
        let total: usize = self.chunks.iter().map(|c| c.len()).sum();
        let mut out = String::with_capacity(total);
        for c in &self.chunks {
            out.push_str(&c.text);
        }
        out
    }

    /// Flat span list: (byte_start, byte_end, fg, bg, attributes) for each
    /// non-empty chunk, in order.
    #[must_use]
    pub fn styled_spans(&self) -> Vec<StyledSpan> {
        let mut spans = Vec::with_capacity(self.chunks.len());
        let mut cursor: usize = 0;
        for c in &self.chunks {
            let len = c.text.len();
            if len == 0 {
                continue;
            }
            spans.push(StyledSpan {
                start: cursor,
                end: cursor + len,
                fg: c.fg,
                bg: c.bg,
                attributes: c.attributes,
            });
            cursor += len;
        }
        spans
    }
}

/// A contiguous byte range `[start, end)` with a single style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledSpan {
    pub start: usize,
    pub end: usize,
    pub fg: Option<Rgba>,
    pub bg: Option<Rgba>,
    pub attributes: u32,
}

/// A highlight applied to a byte range in the buffer
/// (`packages/core/src/types.ts:206` `Highlight`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Highlight {
    /// Byte start (inclusive) into the buffer text.
    pub start: usize,
    /// Byte end (exclusive) into the buffer text.
    pub end: usize,
    pub style_id: u32,
    pub priority: Option<u8>,
    pub hl_ref: Option<u16>,
}

impl Highlight {
    #[must_use]
    pub fn new(start: usize, end: usize, style_id: u32) -> Self {
        Self {
            start,
            end,
            style_id,
            priority: None,
            hl_ref: None,
        }
    }

    #[must_use]
    pub fn with_priority(mut self, p: u8) -> Self {
        self.priority = Some(p);
        self
    }

    #[must_use]
    pub fn with_ref(mut self, r: u16) -> Self {
        self.hl_ref = Some(r);
        self
    }

    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}

/// TextBuffer holds plain text plus ordered styled spans and highlights.
/// Mirrors `packages/core/src/text-buffer.ts` `TextBuffer` semantics for the
/// Rust side: `set_text`/`set_styled_text` replace content and reset spans.
pub struct TextBuffer {
    text: String,
    /// Flat styled spans over `text`, byte offsets.
    spans: Vec<StyledSpan>,
    /// Highlights registered against byte ranges.
    highlights: Vec<Highlight>,
    /// Default foreground (nullable like TS `setDefaultFg`).
    default_fg: Option<Rgba>,
    default_bg: Option<Rgba>,
    default_attributes: u32,
}

impl TextBuffer {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            text: String::new(),
            spans: Vec::new(),
            highlights: Vec::new(),
            default_fg: None,
            default_bg: None,
            default_attributes: 0,
        }
    }

    /// Set plain text. Clears prior styled spans and highlights (mirrors TS
    /// `TextBuffer.setText`).
    pub fn set_text(&mut self, text: &str) -> Result<(), BridgeError> {
        check_text_bytes(text)?;
        self.text = text.to_owned();
        self.spans.clear();
        self.clear_all_highlights();
        Ok(())
    }

    /// Set plain text without byte-cap validation (internal/helper).
    pub fn set_text_unbounded(&mut self, text: &str) -> Result<(), BridgeError> {
        self.text = text.to_owned();
        self.spans.clear();
        self.clear_all_highlights();
        Ok(())
    }

    /// Set styled text from chunks; resets spans/highlights
    /// (mirrors TS `setStyledText`).
    pub fn set_styled_text(&mut self, styled: &StyledText) -> Result<(), BridgeError> {
        let total: usize = styled.chunks.iter().map(|c| c.len()).sum();
        if total > MAX_TEXT_BYTES {
            return Err(BridgeError::TextTooLarge);
        }
        self.spans.clear();
        self.clear_all_highlights();
        let mut out = String::with_capacity(total);
        let mut cursor: usize = 0;
        for c in &styled.chunks {
            out.push_str(&c.text);
            if !c.text.is_empty() {
                self.spans.push(StyledSpan {
                    start: cursor,
                    end: cursor + c.text.len(),
                    fg: c.fg,
                    bg: c.bg,
                    attributes: c.attributes,
                });
                cursor += c.text.len();
            }
        }
        self.text = out;
        Ok(())
    }

    /// Append a styled chunk to existing content.
    pub fn append_chunk(&mut self, chunk: &TextChunk) -> Result<(), BridgeError> {
        if chunk.text.len() > MAX_TEXT_BYTES {
            return Err(BridgeError::TextTooLarge);
        }
        let start = self.text.len();
        self.text.push_str(&chunk.text);
        if !chunk.text.is_empty() {
            self.spans.push(StyledSpan {
                start,
                end: start + chunk.text.len(),
                fg: chunk.fg,
                bg: chunk.bg,
                attributes: chunk.attributes,
            });
        }
        Ok(())
    }

    pub fn add_highlight(&mut self, hl: Highlight) {
        self.highlights.push(hl);
    }

    pub fn remove_highlights_by_ref(&mut self, hl_ref: u16) {
        self.highlights.retain(|h| h.hl_ref != Some(hl_ref));
    }

    pub fn clear_line_highlights(&mut self, line_idx: u32) {
        let line_start = self.line_start_byte(line_idx);
        let line_end = self.line_end_byte(line_idx);
        self.highlights
            .retain(|h| h.end <= line_start || h.start >= line_end);
    }

    pub fn clear_all_highlights(&mut self) {
        self.highlights.clear();
    }

    pub fn get_line_highlights(&self, line_idx: u32) -> Vec<Highlight> {
        let ls = self.line_start_byte(line_idx);
        let le = self.line_end_byte(line_idx);
        self.highlights
            .iter()
            .filter(|h| h.start >= ls && h.start < le)
            .cloned()
            .collect()
    }

    pub fn highlight_count(&self) -> usize {
        self.highlights.len()
    }

    pub fn set_default_fg(&mut self, fg: Option<Rgba>) {
        self.default_fg = fg;
    }
    pub fn set_default_bg(&mut self, bg: Option<Rgba>) {
        self.default_bg = bg;
    }
    pub fn set_default_attributes(&mut self, attrs: Option<u32>) {
        self.default_attributes = attrs.unwrap_or(0);
    }
    pub fn reset_defaults(&mut self) {
        self.default_fg = None;
        self.default_bg = None;
        self.default_attributes = 0;
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.spans.clear();
        self.clear_all_highlights();
    }

    pub fn reset(&mut self) {
        self.clear();
        self.default_fg = None;
        self.default_bg = None;
        self.default_attributes = 0;
    }

    // ---- read accessors ----

    #[must_use]
    pub fn len(&self) -> usize {
        self.text.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn byte_size(&self) -> usize {
        self.text.len()
    }

    #[must_use]
    pub fn spans(&self) -> &[StyledSpan] {
        &self.spans
    }

    #[must_use]
    pub fn highlights(&self) -> &[Highlight] {
        &self.highlights
    }

    #[must_use]
    pub fn default_fg(&self) -> Option<Rgba> {
        self.default_fg
    }
    #[must_use]
    pub fn default_bg(&self) -> Option<Rgba> {
        self.default_bg
    }
    #[must_use]
    pub fn default_attributes(&self) -> u32 {
        self.default_attributes
    }

    /// Line count = number of `\n` + 1 (empty text = 1 line, mirroring the
    /// native `lineCount()` behaviour).
    #[must_use]
    pub fn line_count(&self) -> u32 {
        if self.text.is_empty() {
            return 1;
        }
        1 + self.text.matches('\n').count() as u32
    }

    /// Byte offset of the start of logical line `line_idx` (0-based).
    #[must_use]
    pub fn line_start_byte(&self, line_idx: u32) -> usize {
        if line_idx == 0 {
            return 0;
        }
        let mut idx = 0usize;
        let mut nl_count = 0u32;
        for (i, b) in self.text.bytes().enumerate() {
            if nl_count == line_idx {
                idx = i;
                break;
            }
            if b == b'\n' {
                nl_count += 1;
            }
            idx = i + 1;
        }
        idx
    }

    /// Byte offset just past the end of logical line `line_idx`.
    #[must_use]
    pub fn line_end_byte(&self, line_idx: u32) -> usize {
        let start = self.line_start_byte(line_idx);
        let rest = &self.text[start..];
        match rest.find('\n') {
            Some(pos) => start + pos + 1,
            None => self.text.len(),
        }
    }

    /// Byte range [start, end) for a logical line (end exclusive, no newline).
    #[must_use]
    pub fn line_range(&self, line_idx: u32) -> std::ops::Range<usize> {
        let s = self.line_start_byte(line_idx);
        let mut e = s;
        for b in self.text[s..].bytes() {
            if b == b'\n' {
                break;
            }
            e += 1;
        }
        s..e
    }
}

/// Wrap modes (`packages/native/src/text-buffer.zig` `WrapMode` / TS
/// `text-buffer-view.ts` `setWrapMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapMode {
    None,
    Char,
    Word,
}

/// Result from measuring dimensions without modifying virtual-line cache
/// (`packages/native/src/text-buffer-view.zig:125` `MeasureResult`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MeasureResult {
    pub line_count: u32,
    pub width_cols_max: u32,
}

/// A view over a [`TextBuffer`] with viewport, wrap mode and first-line
/// offset (mirrors `packages/core/src/text-buffer-view.ts`).
pub struct TextBufferView {
    buffer: TextBuffer,
    wrap_mode: WrapMode,
    wrap_width: Option<u32>,
    viewport: Viewport,
    first_line_offset: u32,
}

/// Viewport rectangle in cell columns/rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Viewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Viewport {
    #[must_use]
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    #[must_use]
    pub const fn empty() -> Self {
        Self { x: 0, y: 0, width: 0, height: 0 }
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

impl TextBufferView {
    #[must_use]
    pub const fn new(buffer: TextBuffer) -> Self {
        Self {
            buffer,
            wrap_mode: WrapMode::None,
            wrap_width: None,
            viewport: Viewport::empty(),
            first_line_offset: 0,
        }
    }

    pub fn set_wrap_mode(&mut self, mode: WrapMode) {
        self.wrap_mode = mode;
    }

    pub fn set_wrap_width(&mut self, width: Option<u32>) {
        self.wrap_width = width;
    }

    pub fn set_viewport(&mut self, vp: Viewport) {
        self.viewport = vp;
    }

    pub fn set_viewport_size(&mut self, width: u32, height: u32) {
        self.viewport = Viewport {
            x: self.viewport.x,
            y: self.viewport.y,
            width,
            height,
        };
    }

    pub fn set_first_line_offset(&mut self, offset: u32) {
        self.first_line_offset = offset;
    }

    #[must_use]
    pub fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    #[must_use]
    pub fn buffer_mut(&mut self) -> &mut TextBuffer {
        &mut self.buffer
    }

    #[must_use]
    pub fn wrap_mode(&self) -> WrapMode {
        self.wrap_mode
    }

    #[must_use]
    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// Measure dimensions for given width/height without mutating cache.
    /// Mirrors `measureForDimensions` (`text-buffer-view.zig:1450`).
    /// `width == 0` or `WrapMode::None` => intrinsic max-content width.
    #[must_use]
    pub fn measure_for_dimensions(&self, width: u32, _height: u32) -> MeasureResult {
        let line_count = self.buffer.line_count();
        if width == 0 || self.wrap_mode == WrapMode::None {
            let width_cols_max = self
                .buffer
                .text
                .split('\n')
                .map(display_width_u32)
                .max()
                .unwrap_or(0);
            return MeasureResult {
                line_count,
                width_cols_max,
            };
        }
        // Wrapping path: compute virtual line count + max width by walking
        // each logical line and hard-wrapping on display columns.
        let wrap_cols = width;
        let mut vlines: u32 = 0;
        let mut width_max: u32 = 0;
        for logical in self.buffer.text.split('\n') {
            if logical.is_empty() {
                vlines += 1;
                continue;
            }
            let runs = soft_wrap_display(logical, wrap_cols, self.wrap_mode);
            if runs.is_empty() {
                vlines += 1;
            } else {
                for w in &runs {
                    vlines += 1;
                    if *w > width_max {
                        width_max = *w;
                    }
                }
            }
        }
        MeasureResult {
            line_count: vlines,
            width_cols_max: width_max,
        }
    }

    /// Virtual (wrapped) line count at the current viewport/wrap settings.
    #[must_use]
    pub fn virtual_line_count(&self) -> u32 {
        self.measure_for_dimensions(
            if self.wrap_width == Some(0) || self.wrap_width.is_none() {
                0
            } else {
                self.wrap_width.unwrap_or(0)
            },
            self.viewport.height,
        )
        .line_count
    }

    /// Hard-wrap the entire buffer text at `width` display columns,
    /// splitting on `\n` first (mirrors `wrap_text` plus display width).
    #[must_use]
    pub fn wrap_lines(&self, width: usize) -> Result<Vec<String>, BridgeError> {
        if width == 0 {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for logical in self.buffer.text.split('\n') {
            let mut runs = split_runs_display(logical, width as u32, self.wrap_mode);
            if runs.is_empty() {
                runs.push(String::new());
            }
            out.extend(runs);
        }
        Ok(out)
    }

    /// Clip rows for `render_once`-style draws: rows beyond `rows` skipped,
    /// empty clipped lines skipped, each kept line `(y, clipped)` with at
    /// most `cols` display columns. Uses display width, not char count.
    #[must_use]
    pub fn clip_rows(&self, cols: u32, rows: u32) -> Result<Vec<(u32, String)>, BridgeError> {
        let lines = self.visible_lines(cols);
        let mut out = Vec::new();
        for (y, line) in lines.iter().enumerate().take(rows as usize) {
            let clipped = clip_display(&line, cols as usize);
            if clipped.is_empty() {
                continue;
            }
            out.push((y as u32, clipped));
        }
        Ok(out)
    }

    /// Visible (wrapped) lines within the current viewport, each clipped to
    /// `cols` display columns. Returns the full set (caller bounds rows).
    #[must_use]
    pub fn visible_lines(&self, cols: u32) -> Vec<String> {
        if cols == 0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        let wrap_cols = self.wrap_width.unwrap_or(cols);
        let width = wrap_cols.min(cols);
        for logical in self.buffer.text.split('\n') {
            let mut runs = split_runs_display(logical, width, self.wrap_mode);
            if runs.is_empty() {
                runs.push(String::new());
            }
            out.extend(runs);
        }
        out
    }
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for TextBufferView {
    fn default() -> Self {
        Self::new(TextBuffer::new())
    }
}

/// Display width of a string: CJK=full, combining=0, default=1.
/// Surrogates / invalid UTF-8 handled safely (Rust `&str` is valid UTF-8).
#[must_use]
pub fn display_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

#[must_use]
pub fn display_width_u32(s: &str) -> u32 {
    display_width(s) as u32
}

/// Cell width of a single char. Wide CJK = 2, combining/zero-width = 0,
/// default = 1, control chars = 0 (print as space but 0 cols here).
#[must_use]
pub fn char_width(c: char) -> usize {
    // Zero-width / combining
    if c == '\u{0}' || c.is_control() {
        return 0;
    }
    let cp = c as u32;
    // Hangul Jamo + combining marks + variation selectors are zero-width.
    if is_zero_width(cp) {
        return 0;
    }
    if is_wide(cp) {
        return 2;
    }
    1
}

/// Hard/soft-wrap a single logical line into display-column widths.
/// Returns the per-run display widths (sum of char widths, split at
/// width boundary). Width is in display columns.
#[must_use]
pub fn soft_wrap_display(line: &str, width: u32, mode: WrapMode) -> Vec<u32> {
    if width == 0 {
        return Vec::new();
    }
    let w = width as usize;
    match mode {
        WrapMode::None => {
            let dw = display_width(line);
            if dw == 0 && line.is_empty() {
                vec![0]
            } else {
                vec![dw as u32]
            }
        }
        WrapMode::Char => hard_wrap_display(line, w),
        WrapMode::Word => word_wrap_display(line, w),
    }
}

/// Hard-wrap by display columns (char mode).
#[must_use]
pub fn hard_wrap_display(line: &str, width: usize) -> Vec<u32> {
    if width == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut cols: usize = 0;
    for c in line.chars() {
        let cw = char_width(c);
        if cols + cw > width && cols > 0 {
            out.push(cols as u32);
            cols = cw;
        } else {
            cols += cw;
        }
    }
    if cols > 0 || line.is_empty() {
        out.push(cols as u32);
    }
    out
}

/// Word-wrap by display columns (word mode), breaking long words hard.
#[must_use]
pub fn word_wrap_display(line: &str, width: usize) -> Vec<u32> {
    if width == 0 {
        return Vec::new();
    }
    let mut out: Vec<u32> = Vec::new();
    let mut cols: usize = 0;
    let mut word_cols: usize = 0;
    for c in line.chars() {
        let cw = char_width(c);
        let is_space = c.is_whitespace();
        if is_space {
            if cols + cw > width && cols > 0 {
                out.push(cols as u32);
                cols = 0;
            }
            cols += cw;
            word_cols = 0;
        } else {
            if cols + cw > width {
                if cols == 0 && cw > 0 {
                    // word doesn't fit at all; emit hard break
                    out.push(cols as u32);
                    cols = cw;
                } else if cols > 0 {
                    out.push(cols as u32);
                    cols = cw;
                } else {
                    cols += cw;
                }
            } else {
                cols += cw;
            }
            word_cols += cw;
        }
    }
    if cols > 0 || line.is_empty() || out.is_empty() {
        out.push(cols as u32);
    }
    out
}

/// Split one logical line into wrapped display runs (each at most `width`
/// display columns). Char mode hard-wraps; word mode wraps at spaces;
/// none mode returns the whole line as one run.
#[must_use]
pub fn split_runs_display(line: &str, width: u32, mode: WrapMode) -> Vec<String> {
    let w = width as usize;
    if width == 0 {
        return Vec::new();
    }
    if line.is_empty() {
        return vec![String::new()];
    }
    match mode {
        WrapMode::None => vec![line.to_owned()],
        WrapMode::Char => {
            let mut runs = Vec::new();
            let mut cur = String::new();
            let mut cols: usize = 0;
            for c in line.chars() {
                let cw = char_width(c);
                if cols + cw > w && cols > 0 {
                    runs.push(cur);
                    cur = String::new();
                    cols = 0;
                }
                cur.push(c);
                cols += cw;
            }
            runs.push(cur);
            runs
        }
        WrapMode::Word => {
            let mut runs: Vec<String> = Vec::new();
            let mut cur = String::new();
            let mut cols: usize = 0;
            for word in line.split_inclusive(' ') {
                let ww = display_width(word);
                if cols + ww > w && cols > 0 {
                    runs.push(cur);
                    cur = String::new();
                    cols = 0;
                }
                cur.push_str(word);
                cols += ww;
            }
            runs.push(cur);
            runs
        }
    }
}

/// Split `line` by display columns into the `n`-th wrapped run (0-based),
/// returning the substring for that run.
#[must_use]
pub fn split_by_display(line: &str, width: usize, n: usize) -> String {
    split_runs_display(line, width as u32, WrapMode::Char)
        .into_iter()
        .nth(n)
        .unwrap_or_default()
}

/// Clip `text` to at most `max_cols` display columns (not char count).
#[must_use]
pub fn clip_display(text: &str, max_cols: usize) -> String {
    let mut out = String::new();
    let mut cols: usize = 0;
    for c in text.chars() {
        let cw = char_width(c);
        if cols + cw > max_cols {
            break;
        }
        out.push(c);
        cols += cw;
    }
    out
}

/// CJK / emoji-wide detection (EastAsianWidth W or F, plus emoji).
#[must_use]
pub fn is_wide(cp: u32) -> bool {
    // Hangul syllables
    if (0xAC00..=0xD7A3).contains(&cp) {
        return true;
    }
    // CJK Unified Ideographs
    if (0x4E00..=0x9FFF).contains(&cp) {
        return true;
    }
    // CJK extensions
    if (0x3400..=0x4DBF).contains(&cp)
        || (0x20000..=0x2A6DF).contains(&cp)
        || (0x2A700..=0x2B73F).contains(&cp)
        || (0x2B740..=0x2B8FF).contains(&cp)
        || (0x2CEB0..=0x2EBFF).contains(&cp)
    {
        return true;
    }
    // Fullwidth / wide forms
    if (0xFF01..=0xFF60).contains(&cp) || (0xFFE0..=0xFFE6).contains(&cp) {
        return true;
    }
    // Emoji wide (rough set)
    // Emoji presentation area (wide)
    if (0x1F300..=0x1FAFF).contains(&cp) {
        return true;
    }
    false
}

/// Zero-width detection (combining marks, variation selectors, etc.).
#[must_use]
pub fn is_zero_width(cp: u32) -> bool {
    // C0/C1 control
    if (0x0000..=0x001F).contains(&cp) || (0x007F..=0x009F).contains(&cp) {
        return true;
    }
    // Combining Diacritical Marks
    if (0x0300..=0x036F).contains(&cp) {
        return true;
    }
    // Combining Half Marks
    if (0xFEFF..=0xFEFF).contains(&cp) || (0x0640..=0x0640).contains(&cp) {
        return true;
    }
    // Variation selectors
    if (0xFE00..=0xFE0F).contains(&cp) {
        return true;
    }
    // Zero-width Joiner / Non-Joiner
    if cp == 0x200B || cp == 0x200C || cp == 0x200D || cp == 0x2060 || cp == 0xFEFF {
        return true;
    }
    // Interlinear annotation anchor / a few zero-width chars
    if (0x2060..=0x2064).contains(&cp) {
        return true;
    }
    false
}

/// Style definition input (mirrors `syntax-style.ts` `StyleDefinitionInput`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StyleDefinitionInput {
    pub fg: Option<String>,
    pub bg: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub dim: Option<bool>,
    pub strikethrough: Option<bool>,
    pub blink: Option<bool>,
}

/// Merged style result (`syntax-style.ts` `MergedStyle`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergedStyle {
    pub fg: Option<Rgba>,
    pub bg: Option<Rgba>,
    pub attributes: u32,
}

impl MergedStyle {
    #[must_use]
    pub fn merge_attributes(bold: Option<bool>, italic: Option<bool>, underline: Option<bool>, dim: Option<bool>, strikethrough: Option<bool>, blink: Option<bool>) -> u32 {
        let mut a = attr::NONE;
        if bold == Some(true) { a |= attr::BOLD; }
        if dim == Some(true) { a |= attr::DIM; }
        if italic == Some(true) { a |= attr::ITALIC; }
        if underline == Some(true) { a |= attr::UNDERLINE; }
        if blink == Some(true) { a |= attr::BLINK; }
        if strikethrough == Some(true) { a |= attr::STRIKETHROUGH; }
        a
    }

    /// Merge an ordered list of style *names* (later overrides earlier),
    /// mirroring `SyntaxStyle.mergeStyles` (`syntax-style.ts:mergeStyles`).
    /// Each name maps to a registered definition looked up via `lookup`.
    #[must_use]
    pub fn merge_styles(
        styles: &[(&str, &MergedStyle)],
    ) -> MergedStyle {
        let mut out = MergedStyle::default();
        for (_name, s) in styles {
            if let Some(fg) = s.fg {
                out.fg = Some(fg);
            }
            if let Some(bg) = s.bg {
                out.bg = Some(bg);
            }
            // Attributes override as a whole only when non-zero? No: per-field
            // OR-ing the base bits mirrors TS semantics (flags accumulate).
            out.attributes |= s.attributes & 0xff;
        }
        out
    }
}

/// Resolve a color input string into a packed `Rgba` (mirrors
/// `RGBA.fromValues`/`parseColor`). Hex `#rrggbb` / `#rgb` supported.
/// Fail-closed: returns `None` on unparseable input.
#[must_use]
pub fn parse_rgba(input: &str) -> Option<Rgba> {
    let s = input.trim();
    let digits = s.strip_prefix('#').unwrap_or(s);
    let b = digits.as_bytes();
    let (r, g, bl, a) = match b.len() {
        3 => {
            let hex = |i: usize| -> Option<u8> {
                let v = hex_nibble(b[i])?;
                Some(v << 4 | v)
            };
            (hex(0)?, hex(1)?, hex(2)?, 255)
        }
        6 => {
            let byte_at = |i: usize| -> Option<u8> {
                let hi = hex_nibble(b[i])?;
                let lo = hex_nibble(b[i + 1])?;
                Some(hi << 4 | lo)
            };
            (byte_at(0)?, byte_at(2)?, byte_at(4)?, 255)
        }
        8 => {
            let byte_at = |i: usize| -> Option<u8> {
                let hi = hex_nibble(b[i])?;
                let lo = hex_nibble(b[i + 1])?;
                Some(hi << 4 | lo)
            };
            (byte_at(0)?, byte_at(2)?, byte_at(4)?, byte_at(6)?)
        }
        _ => return None,
    };
    Some(Rgba::new(r, g, bl, a))
}

#[must_use]
fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Build a `MergedStyle` from a `StyleDefinitionInput`.
#[must_use]
pub fn merge_style_definition(def: &StyleDefinitionInput) -> MergedStyle {
    MergedStyle {
        fg: def.fg.as_deref().and_then(parse_rgba),
        bg: def.bg.as_deref().and_then(parse_rgba),
        attributes: MergedStyle::merge_attributes(
            def.bold,
            def.italic,
            def.underline,
            def.dim,
            def.strikethrough,
            def.blink,
        ),
    }
}

/// Measure intrinsic (max-content) display width of a buffer's text.
#[must_use]
pub fn measure_text_width(text: &str) -> u32 {
    text.split('\n')
        .map(display_width_u32)
        .max()
        .unwrap_or(0)
}

/// Clip text to at most `max_chars` chars (char-count, NOT display width).
/// Kept for parity with the prior `clip_chars` helper.
#[must_use]
pub fn clip_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

/// Hard-wrap `text` at `width` chars per line, splitting on `\n` first.
#[must_use]
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for logical in text.split('\n') {
        let chars: Vec<char> = logical.chars().collect();
        if chars.is_empty() {
            out.push(String::new());
            continue;
        }
        for chunk in chars.chunks(width) {
            out.push(chunk.iter().collect());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- 1. set_text / set_styled_text ----------

    #[test]
    fn set_text_basic() {
        let mut tb = TextBuffer::new();
        tb.set_text("hello world").unwrap();
        assert_eq!(tb.text(), "hello world");
        assert_eq!(tb.len(), 11);
        assert!(tb.spans().is_empty());
        assert_eq!(tb.byte_size(), 11);
    }

    #[test]
    fn set_styled_text_preserves_spans() {
        let mut tb = TextBuffer::new();
        let st = StyledText::new(vec![
            TextChunk {
                text: "hi".into(),
                fg: Some(Rgba::rgb(1, 0, 0)),
                bg: None,
                attributes: attr::BOLD,
                link: None,
            },
            TextChunk {
                text: "bye".into(),
                fg: Some(Rgba::rgb(0, 0, 1)),
                bg: Some(Rgba::rgb(0, 0, 0)),
                attributes: attr::UNDERLINE,
                link: None,
            },
        ]);
        tb.set_styled_text(&st).unwrap();
        assert_eq!(tb.text(), "hibye");
        assert_eq!(tb.spans().len(), 2);
        assert_eq!(tb.spans()[0].start, 0);
        assert_eq!(tb.spans()[0].end, 2);
        assert_eq!(tb.spans()[0].fg, Some(Rgba::rgb(1, 0, 0)));
        assert_eq!(tb.spans()[0].attributes, attr::BOLD);
        assert_eq!(tb.spans()[1].start, 2);
        assert_eq!(tb.spans()[1].end, 5);
        assert_eq!(tb.spans()[1].fg, Some(Rgba::rgb(0, 0, 1)));
        assert_eq!(tb.spans()[1].attributes, attr::UNDERLINE);
        // to_plain_text reconstructs the concatenation
        assert_eq!(st.to_plain_text(), "hibye");
    }

    #[test]
    fn set_text_resets_spans_and_highlights() {
        let mut tb = TextBuffer::new();
        let _ = tb.set_styled_text(&StyledText::from_string("abc"));
        tb.add_highlight(Highlight::new(0, 3, 1));
        tb.set_text("xyz").unwrap();
        assert!(tb.spans().is_empty());
        assert!(tb.highlights().is_empty());
    }

    // ---------- 2. set_styled_text ----------

    #[test]
    fn set_styled_text_rejects_oversized() {
        let mut tb = TextBuffer::new();
        let big = "x".repeat(MAX_TEXT_BYTES + 1);
        let st = StyledText::from_string(&big);
        assert_eq!(
            tb.set_styled_text(&st).unwrap_err(),
            BridgeError::TextTooLarge
        );
        assert_eq!(tb.text(), "");
    }

    #[test]
    fn set_text_rejects_oversized() {
        let mut tb = TextBuffer::new();
        let big = "x".repeat(MAX_TEXT_BYTES + 1);
        assert_eq!(
            tb.set_text(&big).unwrap_err(),
            BridgeError::TextTooLarge
        );
    }

    // ---------- 3. highlights ----------

    #[test]
    fn highlights_add_get_remove() {
        let mut tb = TextBuffer::new();
        tb.set_text_unbounded("hello world");
        tb.add_highlight(Highlight::new(0, 5, 1).with_ref(10));
        tb.add_highlight(Highlight::new(6, 11, 2));
        assert_eq!(tb.highlight_count(), 2);

        let line0 = tb.get_line_highlights(0);
        assert_eq!(line0.len(), 2);

        tb.remove_highlights_by_ref(10);
        assert_eq!(tb.highlight_count(), 1);
        assert_eq!(tb.highlights()[0].style_id, 2);
    }

    #[test]
    fn highlight_intersects() {
        let a = Highlight::new(0, 5, 0);
        let b = Highlight::new(3, 8, 0);
        let c = Highlight::new(6, 9, 0);
        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }

    #[test]
    fn line_highlights_filtered_to_line() {
        let mut tb = TextBuffer::new();
        tb.set_text_unbounded("line0\nline1\nline2");
        // byte offsets: line0=[0,5] (+nl at 5), line1=[6,11] (+nl at 11), line2=[12,17]
        tb.add_highlight(Highlight::new(0, 5, 1)); // line 0
        tb.add_highlight(Highlight::new(7, 11, 2)); // line 1
        tb.add_highlight(Highlight::new(13, 17, 3)); // line 2

        let l0 = tb.get_line_highlights(0);
        assert_eq!(l0.len(), 1);
        assert_eq!(l0[0].style_id, 1);

        let l1 = tb.get_line_highlights(1);
        assert_eq!(l1.len(), 1);
        assert_eq!(l1[0].style_id, 2);

        let l2 = tb.get_line_highlights(2);
        assert_eq!(l2.len(), 1);
        assert_eq!(l2[0].style_id, 3);

        // clearing line 1 removes only its highlight
        tb.clear_line_highlights(1);
        assert_eq!(tb.highlight_count(), 2);
        assert_eq!(tb.get_line_highlights(1).len(), 0);
    }

    // ---------- 4. wrap ----------

    #[test]
    fn wrap_text_hard_wrap_chars() {
        assert_eq!(wrap_text("abcdef", 2), vec!["ab", "cd", "ef"]);
        assert_eq!(wrap_text("ab\ncde", 2), vec!["ab", "cd", "e"]);
        assert_eq!(wrap_text("", 4), vec![""]);
        assert!(wrap_text("abc", 0).is_empty());
    }

    #[test]
    fn wrap_view_char_mode_display_width() {
        let mut tb = TextBuffer::new();
        tb.set_text_unbounded("ABCDEF");
        let mut view = TextBufferView::new(tb);
        view.set_wrap_mode(WrapMode::Char);
        let lines = view.wrap_lines(2).unwrap();
        assert_eq!(lines, vec!["AB", "CD", "EF"]);
    }

    #[test]
    fn wrap_view_cjk_display_width_not_char_count() {
        // "一二" = 2 chars but 4 display columns. Wrapping at 2 cols => 1 char per line.
        let mut tb = TextBuffer::new();
        tb.set_text_unbounded("一二");
        let mut view = TextBufferView::new(tb);
        view.set_wrap_mode(WrapMode::Char);
        let lines = view.wrap_lines(2).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "一");
        assert_eq!(lines[1], "二");
    }

    // ---------- 5. viewport ----------

    #[test]
    fn clip_rows_skips_beyond_rows() {
        let mut tb = TextBuffer::new();
        tb.set_text_unbounded("hello\nworld");
        let view = TextBufferView::new(tb);
        let rows = view.clip_rows(5, 1).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0], (0, "hello".to_owned()));
    }

    #[test]
    fn clip_rows_clips_to_cols_display_width() {
        let mut tb = TextBuffer::new();
        tb.set_text_unbounded("hello world");
        let view = TextBufferView::new(tb);
        let rows = view.clip_rows(5, 10).unwrap();
        assert_eq!(rows[0], (0, "hello".to_owned()));
    }

    #[test]
    fn clip_rows_skips_empty_clipped_lines() {
        let mut tb = TextBuffer::new();
        tb.set_text_unbounded("hi\n\nworld");
        let view = TextBufferView::new(tb);
        let rows = view.clip_rows(10, 10).unwrap();
        // line 1 is empty -> skipped
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], (0, "hi".to_owned()));
        assert_eq!(rows[1], (2, "world".to_owned()));
    }

    // ---------- 6. measure_for_dimensions ----------

    #[test]
    fn measure_for_dimensions_no_wrap_intrinsic() {
        let mut tb = TextBuffer::new();
        tb.set_text_unbounded("hello\nworld!");
        let view = TextBufferView::new(tb);
        let m = view.measure_for_dimensions(0, 24);
        assert_eq!(m.line_count, 2);
        assert_eq!(m.width_cols_max, 6);
    }

    #[test]
    fn measure_for_dimensions_char_wrap() {
        let mut tb = TextBuffer::new();
        tb.set_text_unbounded("ABCDEF");
        let mut view = TextBufferView::new(tb);
        view.set_wrap_mode(WrapMode::Char);
        let m = view.measure_for_dimensions(2, 24);
        assert_eq!(m.line_count, 3);
        assert_eq!(m.width_cols_max, 2);
    }

    #[test]
    fn measure_for_dimensions_none_wrap_ignores_width() {
        let mut tb = TextBuffer::new();
        tb.set_text_unbounded("hello world");
        let mut view = TextBufferView::new(tb);
        view.set_wrap_mode(WrapMode::None);
        let m = view.measure_for_dimensions(5, 24);
        // None mode => intrinsic, single line
        assert_eq!(m.line_count, 1);
        assert_eq!(m.width_cols_max, 11);
    }

    // ---------- 7. merge_styles ----------

    #[test]
    fn merge_styles_override_semantics() {
        let base = MergedStyle {
            fg: Some(Rgba::rgb(255, 0, 0)),
            bg: None,
            attributes: attr::BOLD,
        };
        let over = MergedStyle {
            fg: Some(Rgba::rgb(0, 255, 0)),
            bg: Some(Rgba::rgb(0, 0, 0)),
            attributes: attr::UNDERLINE,
        };
        let merged = MergedStyle::merge_styles(&[("base", &base), ("over", &over)]);
        assert_eq!(merged.fg, Some(Rgba::rgb(0, 255, 0)));
        assert_eq!(merged.bg, Some(Rgba::rgb(0, 0, 0)));
        assert_eq!(merged.attributes, attr::BOLD | attr::UNDERLINE);

        // Single style
        let only = MergedStyle::merge_styles(&[("x", &base)]);
        assert_eq!(only.fg, Some(Rgba::rgb(255, 0, 0)));
        assert_eq!(only.bg, None);
        assert_eq!(only.attributes, attr::BOLD);

        // Empty
        let empty = MergedStyle::merge_styles(&[]);
        assert_eq!(empty.fg, None);
        assert_eq!(empty.bg, None);
        assert_eq!(empty.attributes, 0);
    }

    #[test]
    fn merge_from_style_definition() {
        let def = StyleDefinitionInput {
            fg: Some("#ff0000".into()),
            bg: Some("#0000ff".into()),
            bold: Some(true),
            underline: Some(true),
            ..Default::default()
        };
        let m = merge_style_definition(&def);
        assert_eq!(m.fg, Some(Rgba::rgb(255, 0, 0)));
        assert_eq!(m.bg, Some(Rgba::rgb(0, 0, 255)));
        assert_eq!(m.attributes, attr::BOLD | attr::UNDERLINE);
    }

    #[test]
    fn parse_rgba_hex_forms() {
        assert_eq!(parse_rgba("#ff0000"), Some(Rgba::rgb(255, 0, 0)));
        assert_eq!(parse_rgba("#f00"), Some(Rgba::rgb(255, 0, 0)));
        assert_eq!(parse_rgba("ff0000"), Some(Rgba::rgb(255, 0, 0)));
        assert_eq!(parse_rgba("#00ff0080"), Some(Rgba::new(0, 255, 0, 128)));
        assert_eq!(parse_rgba("red"), None);
        assert_eq!(parse_rgba("#gg"), None);
    }

    // ---------- 8. display-width clip, not char count ----------

    #[test]
    fn clip_display_is_display_width_not_char_count() {
        // "一二三" = 3 chars, 6 display cols. Clip at 3 cols => "一" (2 cols; 二 would exceed).
        let s = "一二三";
        let clipped = clip_display(s, 3);
        assert_eq!(clipped, "一");
        // Clip at 2 cols => just "一" (a single wide char = 2 cols).
        assert_eq!(clip_display(s, 2), "一");
        // Clip at 5 cols => "一二" (4 cols) can't fit 三 (would be 6).
        assert_eq!(clip_display(s, 5), "一二");
        // Clip at 6 => all.
        assert_eq!(clip_display(s, 6), "一二三");
    }

    #[test]
    fn display_width_cjk_wide() {
        assert_eq!(display_width("hello"), 5);
        assert_eq!(display_width("一二"), 4);
        assert_eq!(display_width("a一"), 3);
        // combining char: width 0
        assert_eq!(display_width("e\u{0301}"), 1); // e + combining acute
    }

    #[test]
    fn char_width_wide_and_zero() {
        assert_eq!(char_width('A'), 1);
        assert_eq!(char_width('一'), 2);
        assert_eq!(char_width('\u{0301}'), 0); // combining acute
        assert_eq!(char_width('\t'), 0);
        assert_eq!(char_width('\n'), 0);
    }

    #[test]
    fn word_wrap_display_simple() {
        let runs = word_wrap_display("the quick brown", 9);
        // "the quick" = 8 cols (space included), "brown"=5
        let total: u32 = runs.iter().sum();
        assert!(total >= 8);
        let runs2 = word_wrap_display("abcdefg", 3);
        // long word hard-broken into 3-col runs
        assert!(runs2.iter().all(|&w| w <= 3));
    }

    #[test]
    fn rgba_packed_roundtrip() {
        let c = Rgba::rgb(10, 20, 30);
        let packed = c.to_packed();
        assert_eq!(Rgba::from_packed(packed), c);
        assert_eq!(packed, [10, 20, 30, 255]);
    }

    #[test]
    fn text_buffer_view_setters() {
        let mut tb = TextBuffer::new();
        tb.set_text_unbounded("hi");
        let mut view = TextBufferView::new(tb);
        view.set_wrap_mode(WrapMode::Word);
        view.set_viewport_size(80, 24);
        view.set_wrap_width(Some(40));
        view.set_first_line_offset(2);
        assert_eq!(view.wrap_mode(), WrapMode::Word);
        assert_eq!(view.viewport(), Viewport::new(0, 0, 80, 24));
        assert_eq!(view.virtual_line_count(), 1);
    }

    #[test]
    fn append_chunk_grows_spans() {
        let mut tb = TextBuffer::new();
        tb.set_text_unbounded("ab").unwrap();
        tb.append_chunk(&TextChunk {
            text: "cd".into(),
            fg: Some(Rgba::rgb(1, 2, 3)),
            bg: None,
            attributes: attr::ITALIC,
            link: None,
        })
        .unwrap();
        assert_eq!(tb.text(), "abcd");
        assert_eq!(tb.spans().len(), 1);
        assert_eq!(tb.spans()[0].start, 2);
        assert_eq!(tb.spans()[0].end, 4);
    }

    #[test]
    fn styled_text_new_from_string_empty() {
        let st = StyledText::from_string("");
        assert!(st.is_empty());
        assert_eq!(st.to_plain_text(), "");
        assert!(st.styled_spans().is_empty());
    }
}
