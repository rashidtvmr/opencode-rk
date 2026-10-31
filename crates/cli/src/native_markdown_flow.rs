#![forbid(unsafe_code)]
//! Native Markdown block flow: block layout, fence styling, inline style spans,
//! grid tables, conceal/source projection and streaming reconciliation.
//!
//! Evidence pins
//! -------------
//! * OpenCode TUI pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`:
//!   - `packages/tui/src/routes/session/message-parts.tsx:183-196` (assistant
//!     text part: `streaming`, `internalBlockMode="top-level"`,
//!     `tableOptions={{ style: "grid", cellPaddingX: 1 }}`,
//!     `conceal={markdownMode === "rendered"}`, `fg={theme.markdown.text}`).
//!   - `packages/tui/src/routes/session/index.tsx:2222-2231` (compaction body
//!     with the same block/table/conceal configuration).
//!   - `packages/tui/src/routes/session/index.tsx:244` (`session.markdown`
//!     defaults to `"rendered"`).
//!   - `packages/tui/src/plugin/markdown.ts:5-17` (`createMarkdownRenderer`:
//!     last-wins handler map, `isShallowEqual`, renderer memo identity).
//!   - `packages/tui/src/plugin/context.tsx:141-143` (only *active*
//!     registrations contribute handlers).
//!   - `packages/theme/src/tui/syntax.ts:46-71` (`markup.*` scopes, token
//!     colors and attributes: heading bold, h1 bold+underline, strong bold,
//!     italic, list, quote italic, raw, link underline, label).
//!   - `packages/theme/src/tui/v1-migrate.ts:202-217` (`markdown.*` tokens).
//!   - `packages/tui/src/routes/session/thinking-syntax.ts:3-8` (muted
//!     foreground with retained attributes).
//! * @opentui/core 0.6.8 (the version pinned by the parity kit). The package is
//!   not vendored in the repository and only `libopentui.dylib/.so` are present;
//!   behaviour below is taken from the shipped 0.6.8 TypeScript sources
//!   recovered from `index.node.js.map` (`src/renderables/Markdown.ts`,
//!   `src/renderables/markdown-parser.ts`, `src/renderables/TextTable.ts`,
//!   `src/renderables/text-table-width.ts`) and cross-checked against the
//!   published `renderables/Markdown.d.ts`. Line numbers refer to those
//!   original sources:
//!   - `Markdown.ts:592-715` inline chunk rendering and conceal rules.
//!   - `Markdown.ts:716-746` nested inline style (innermost style wins for text).
//!   - `Markdown.ts:856-887` blockquote: left border + one-cell padding, content
//!     parsed as markdown with `markup.quote` base style.
//!   - `Markdown.ts:888-1005` list layout: marker width, `padStart`, content
//!     column, nested list children, loose-item bottom margin.
//!   - `Markdown.ts:1159-1170` horizontal rule: one full-width top border row.
//!   - `Markdown.ts:1171-1186` fenced code: content is `token.text` (fence lines
//!     and info string are not painted).
//!   - `Markdown.ts:1253-1267` block separation and inter-block margins.
//!   - `Markdown.ts:1341-1374` top-level blocks, `space` tokens as gaps,
//!     `shouldAddTopLevelMargin`.
//!   - `Markdown.ts:1430-1461` table cell chunks (empty cell => one space,
//!     header cells adopt the heading style).
//!   - `Markdown.ts:1463-1569` table content cache and option resolution.
//!   - `Markdown.ts:1995-2085` top-level block reconciliation (same raw+type
//!     keeps the block; otherwise update/rebuild in place).
//!   - `Markdown.ts:2170-2192` incremental parse (`trailingUnstable = 2` while
//!     streaming, `0` when finished).
//!   - `markdown-parser.ts:13-79` token prefix reuse by `token.raw`.
//!   - `TextTable.ts:663-750` column sizing (intrinsic width, full expansion,
//!     proportional fit).
//!   - `TextTable.ts:904-931` border/offset math (`n + 1` vertical borders).
//!   - `TextTable.ts:955-968` border layout (outer/inner vertical/horizontal).
//!   - `text-table-width.ts:15-97` proportional column fit.
//!
//! Scope: this module produces block rows (text plus style role per span) for
//! the view to paint. It does not implement mermaid/math fences (package P13b)
//! and does not paint syntax-highlight tokens inside fences; a fence row carries
//! the `CodeBlock` role. Inline emphasis flanking is a conservative subset of
//! marked's delimiter rules; nested `***x***` degrades to strong plus literal
//! `*`. The painter/chat view integration lives in the view modules.
#![allow(dead_code)]

use crate::native_view_layout::visit_unpadded_cells;

/// Bounded Markdown input; longer content is clipped on a char boundary and
/// flagged through [`MarkdownFlow::truncated`].
pub const MAX_FLOW_BYTES: usize = 64 * 1024;
/// Maximum retained top-level blocks.
pub const MAX_BLOCKS: usize = 256;
/// Maximum table columns per table.
pub const MAX_TABLE_COLUMNS: usize = 24;
/// Maximum table body rows per table.
pub const MAX_TABLE_ROWS: usize = 128;
/// Maximum inline spans per rendered line.
pub const MAX_INLINE_SPANS: usize = 128;
/// Upstream `parseMarkdownIncremental(..., trailingUnstable)` while streaming
/// (`Markdown.ts:2177`).
pub const TRAILING_UNSTABLE_TOKENS: usize = 2;

/// Style roles correspond 1:1 to upstream `markup.*` scopes / `markdown.*`
/// theme tokens (`packages/theme/src/tui/syntax.ts:46-71`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MarkdownRole {
    Text,
    Heading,
    Heading1,
    Strong,
    Emphasis,
    Strikethrough,
    InlineCode,
    CodeBlock,
    Link,
    LinkUrl,
    LinkLabel,
    BlockQuote,
    ListMarker,
    Rule,
}

impl MarkdownRole {
    pub const ALL: [Self; 14] = [
        Self::Text,
        Self::Heading,
        Self::Heading1,
        Self::Strong,
        Self::Emphasis,
        Self::Strikethrough,
        Self::InlineCode,
        Self::CodeBlock,
        Self::Link,
        Self::LinkUrl,
        Self::LinkLabel,
        Self::BlockQuote,
        Self::ListMarker,
        Self::Rule,
    ];

    /// Upstream syntax scope for this role.
    #[must_use]
    pub fn scope(self) -> &'static str {
        match self {
            Self::Text => "default",
            Self::Heading => "markup.heading",
            Self::Heading1 => "markup.heading.1",
            Self::Strong => "markup.strong",
            Self::Emphasis => "markup.italic",
            Self::Strikethrough => "markup.strikethrough",
            Self::InlineCode => "markup.raw.inline",
            Self::CodeBlock => "markup.raw.block",
            Self::Link => "markup.link",
            Self::LinkUrl => "markup.link.url",
            Self::LinkLabel => "markup.link.label",
            Self::BlockQuote => "markup.quote",
            Self::ListMarker => "markup.list",
            Self::Rule => "markup.horizontalrule",
        }
    }

    /// Upstream `theme.markdown.*` token this role is painted from.
    #[must_use]
    pub fn token(self) -> &'static str {
        match self {
            Self::Text => "markdown.text",
            Self::Heading | Self::Heading1 => "markdown.heading",
            Self::Strong => "markdown.strong",
            Self::Emphasis => "markdown.emphasis",
            Self::Strikethrough => "text.muted",
            Self::InlineCode | Self::CodeBlock => "markdown.code",
            Self::Link | Self::LinkUrl => "markdown.link",
            Self::LinkLabel => "markdown.linkText",
            Self::BlockQuote => "markdown.blockQuote",
            Self::ListMarker => "markdown.listItem",
            Self::Rule => "markdown.horizontalRule",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Text => 0,
            Self::Heading => 1,
            Self::Heading1 => 2,
            Self::Strong => 3,
            Self::Emphasis => 4,
            Self::Strikethrough => 5,
            Self::InlineCode => 6,
            Self::CodeBlock => 7,
            Self::Link => 8,
            Self::LinkUrl => 9,
            Self::LinkLabel => 10,
            Self::BlockQuote => 11,
            Self::ListMarker => 12,
            Self::Rule => 13,
        }
    }
}

/// Non-color emphasis attributes (OpenTUI `TextAttributes` subset).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StyleAttrs {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dim: bool,
}

impl StyleAttrs {
    /// Bit mask compatible with the CLI attribute constants.
    #[must_use]
    pub fn mask(self) -> u32 {
        u32::from(self.bold)
            | (u32::from(self.italic) << 1)
            | (u32::from(self.underline) << 2)
            | (u32::from(self.dim) << 3)
    }
}

/// One resolved role style. `fg`/`bg` are RGBA bytes supplied by the caller
/// from the active `MarkdownPalette`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct StyleSpec {
    pub fg: [u8; 4],
    pub bg: Option<[u8; 4]>,
    pub attrs: StyleAttrs,
}

/// Role style table. Defaults encode the attributes upstream derives from
/// `packages/theme/src/tui/syntax.ts:46-71`; colors are injected by the caller
/// (the chat view owns the resolved theme palette).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownStyles {
    roles: [StyleSpec; 14],
}

impl Default for MarkdownStyles {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownStyles {
    #[must_use]
    pub fn new() -> Self {
        let mut roles = [StyleSpec::default(); 14];
        let mut set = |role: MarkdownRole, bold: bool, italic: bool, underline: bool| {
            roles[role.index()].attrs = StyleAttrs { bold, italic, underline, dim: false };
        };
        set(MarkdownRole::Heading, true, false, false);
        set(MarkdownRole::Heading1, true, false, true);
        set(MarkdownRole::Strong, true, false, false);
        set(MarkdownRole::Emphasis, false, true, false);
        set(MarkdownRole::BlockQuote, false, true, false);
        set(MarkdownRole::Link, false, false, true);
        set(MarkdownRole::LinkUrl, false, false, true);
        set(MarkdownRole::LinkLabel, false, false, true);
        Self { roles }
    }

    #[must_use]
    pub fn get(&self, role: MarkdownRole) -> StyleSpec {
        self.roles[role.index()]
    }

    pub fn set(&mut self, role: MarkdownRole, spec: StyleSpec) {
        self.roles[role.index()] = spec;
    }

    pub fn set_fg(&mut self, role: MarkdownRole, fg: [u8; 4]) {
        self.roles[role.index()].fg = fg;
    }

    /// `generateThinkingSyntax` (`routes/session/thinking-syntax.ts:3-8`):
    /// every registered style keeps its non-color attributes and adopts the
    /// muted foreground.
    #[must_use]
    pub fn thinking(&self, muted: [u8; 4]) -> Self {
        let mut next = self.clone();
        for spec in &mut next.roles {
            spec.fg = muted;
        }
        next
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TableStyle {
    #[default]
    Grid,
    Columns,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TableWidthMode {
    Content,
    #[default]
    Full,
}

/// Table options mirroring the chat use site
/// (`tableOptions={{ style: "grid", cellPaddingX: 1 }}`) plus the renderable
/// defaults resolved at `Markdown.ts:1530-1569`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableOptions {
    pub style: TableStyle,
    pub width_mode: TableWidthMode,
    pub cell_padding_x: usize,
    pub borders: bool,
    pub wrap: bool,
}

impl Default for TableOptions {
    fn default() -> Self {
        Self {
            style: TableStyle::Grid,
            width_mode: TableWidthMode::Full,
            cell_padding_x: 1,
            borders: true,
            wrap: true,
        }
    }
}

/// Render configuration matching the pinned chat use sites.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkdownOptions {
    /// `streaming` prop. Only trailing-token stability bookkeeping depends on
    /// it; the final parsed blocks are identical either way.
    pub streaming: bool,
    /// `conceal={markdownMode === "rendered"}`.
    pub conceal: bool,
    /// `concealCode` (defaults false upstream).
    pub conceal_code: bool,
    /// Terminal hyperlink capability. OC2 strips OSC-8 hyperlinks
    /// (`crates/cli/src/native_transcript.rs:162`), so links are not clickable
    /// and the concealed form keeps the target in parentheses
    /// (`Markdown.ts:666-680`).
    pub hyperlinks: bool,
    pub table: TableOptions,
}

impl Default for MarkdownOptions {
    fn default() -> Self {
        Self {
            streaming: false,
            conceal: true,
            conceal_code: false,
            hyperlinks: false,
            table: TableOptions::default(),
        }
    }
}

/// A style span inside one rendered line, in displayed-text byte offsets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineSpan {
    pub start: usize,
    pub end: usize,
    pub role: MarkdownRole,
}

/// One logical inline line: displayed text plus style spans.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InlineLine {
    pub text: String,
    pub spans: Vec<InlineSpan>,
}

impl InlineLine {
    #[must_use]
    pub fn plain(text: impl Into<String>) -> Self {
        let text = text.into();
        let spans = if text.is_empty() {
            Vec::new()
        } else {
            vec![InlineSpan { start: 0, end: text.len(), role: MarkdownRole::Text }]
        };
        Self { text, spans }
    }

    #[must_use]
    pub fn cell_width(&self) -> usize {
        cells(&self.text)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListItem {
    pub marker: String,
    pub marker_width: usize,
    pub lines: Vec<InlineLine>,
    pub children: Vec<ListItem>,
    pub loose_after: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Table {
    pub header: Vec<InlineLine>,
    pub rows: Vec<Vec<InlineLine>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Paragraph,
    Heading(u8),
    Code { lang: Option<String> },
    List { ordered: bool, start: usize },
    BlockQuote,
    Rule,
    Table,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockBody {
    Inline(Vec<InlineLine>),
    Code { lines: Vec<String>, lang: Option<String> },
    List(Vec<ListItem>),
    Quote(Vec<InlineLine>),
    Rule,
    Table(Table),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownBlock {
    /// Stable identity: top-level position, matching upstream block state.
    pub id: usize,
    pub kind: BlockKind,
    /// Token raw byte range in the flow content.
    pub source_start: usize,
    pub source_end: usize,
    /// Exact token raw (includes trailing newlines / fence lines).
    pub source_raw: String,
    /// `marginTop` from `shouldAddTopLevelMargin` (`Markdown.ts:1366-1374`).
    pub margin_top: usize,
    /// True when this update kept the previous block's raw text and kind
    /// (`Markdown.ts:2008-2035`).
    pub preserved: bool,
    /// Effective custom fence handler id for a code block, if the plugin
    /// registry currently provides one for `lang`.
    pub fence_handler: Option<String>,
    pub body: BlockBody,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowSpan {
    pub text: String,
    pub role: MarkdownRole,
}

impl RowSpan {
    #[must_use]
    pub fn new(text: impl Into<String>, role: MarkdownRole) -> Self {
        Self { text: text.into(), role }
    }
}

/// One terminal row of the flow. `spans` empty means a blank/margin row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowRow {
    pub block: usize,
    pub spans: Vec<RowSpan>,
}

impl FlowRow {
    #[must_use]
    pub fn blank(block: usize) -> Self {
        Self { block, spans: Vec::new() }
    }

    #[must_use]
    pub fn text(&self) -> String {
        self.spans.iter().map(|span| span.text.as_str()).collect()
    }

    #[must_use]
    pub fn cells(&self) -> usize {
        cells(&self.text())
    }
}

/// Last-wins Markdown code-fence handler registry
/// (`packages/tui/src/plugin/markdown.ts:5-17`,
/// `plugin/context.tsx:141-143`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarkdownPluginRegistry {
    registrations: Vec<Registration>,
    effective: Vec<(String, String)>,
    version: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Registration {
    plugin: String,
    active: bool,
    handlers: Vec<(String, String)>,
}

impl MarkdownPluginRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a plugin (idempotent). A plugin starts active, mirroring a
    /// plugin that just registered.
    pub fn add_plugin(&mut self, plugin: &str) {
        if self.registrations.iter().any(|entry| entry.plugin == plugin) {
            return;
        }
        self.registrations.push(Registration {
            plugin: plugin.to_owned(),
            active: true,
            handlers: Vec::new(),
        });
        self.recompute();
    }

    /// Enable/disable a plugin. Toggling a plugin that contributes no effective
    /// handler must not change the renderer identity (`markdown.ts:8-14`).
    pub fn set_active(&mut self, plugin: &str, active: bool) {
        if let Some(entry) = self.registrations.iter_mut().find(|entry| entry.plugin == plugin) {
            entry.active = active;
            self.recompute();
        }
    }

    /// Set/replace one language handler for a plugin (last registration wins in
    /// iteration order, like `Object.fromEntries(flatMap(entries))`).
    pub fn set_handler(&mut self, plugin: &str, language: &str, handler: &str) {
        if let Some(entry) = self.registrations.iter_mut().find(|entry| entry.plugin == plugin) {
            if let Some(slot) = entry.handlers.iter_mut().find(|(lang, _)| lang == language) {
                slot.1 = handler.to_owned();
            } else {
                entry.handlers.push((language.to_owned(), handler.to_owned()));
            }
            self.recompute();
        }
    }

    pub fn remove_handler(&mut self, plugin: &str, language: &str) {
        if let Some(entry) = self.registrations.iter_mut().find(|entry| entry.plugin == plugin) {
            entry.handlers.retain(|(lang, _)| lang != language);
            self.recompute();
        }
    }

    /// Effective (last-wins) language -> handler map.
    #[must_use]
    pub fn effective(&self) -> &[(String, String)] {
        &self.effective
    }

    /// Changes only when the effective map deep-equals differently
    /// (`isShallowEqual` memo semantics).
    #[must_use]
    pub fn version(&self) -> u64 {
        self.version
    }

    #[must_use]
    pub fn handler_for(&self, language: &str) -> Option<&str> {
        self.effective
            .iter()
            .find(|(lang, _)| lang == language)
            .map(|(_, handler)| handler.as_str())
    }

    fn recompute(&mut self) {
        let mut next: Vec<(String, String)> = Vec::new();
        for registration in self.registrations.iter().filter(|entry| entry.active) {
            for (lang, handler) in &registration.handlers {
                if let Some(slot) = next.iter_mut().find(|(key, _)| key == lang) {
                    slot.1.clone_from(handler);
                } else {
                    next.push((lang.clone(), handler.clone()));
                }
            }
        }
        if next != self.effective {
            self.effective = next;
            self.version = self.version.wrapping_add(1);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RawKind {
    Space,
    Paragraph,
    Heading,
    Code,
    List,
    Quote,
    Rule,
    Table,
}

#[derive(Clone, Debug)]
struct RawToken {
    kind: RawKind,
    raw: String,
    start: usize,
}

#[derive(Clone, Copy, Debug)]
struct LineSpan {
    start: usize,
    /// End offset including the line terminator.
    end: usize,
    /// End offset of the visible text (terminator and `\r` excluded).
    text_end: usize,
}

/// Streaming Markdown flow. Content updates reconcile top-level blocks; the
/// final render is identical for both event orders
/// (`set_content(final) -> finish()` and `finish() -> set_content(final)`)
/// because `updateBlocks` always re-parses the current content
/// (`Markdown.ts:430-437`, `Markdown.ts:2170-2192`).
#[derive(Clone, Debug)]
pub struct MarkdownFlow {
    content: String,
    options: MarkdownOptions,
    styles: MarkdownStyles,
    blocks: Vec<MarkdownBlock>,
    tokens: Vec<RawToken>,
    fence_handlers: Vec<(String, String)>,
    renderer_epoch: u64,
    reused_tokens: usize,
    truncated: bool,
    pending_utf8: Vec<u8>,
}

impl MarkdownFlow {
    #[must_use]
    pub fn new(options: MarkdownOptions) -> Self {
        Self {
            content: String::new(),
            options,
            styles: MarkdownStyles::new(),
            blocks: Vec::new(),
            tokens: Vec::new(),
            fence_handlers: Vec::new(),
            renderer_epoch: 0,
            reused_tokens: 0,
            truncated: false,
            pending_utf8: Vec::new(),
        }
    }

    #[must_use]
    pub fn options(&self) -> MarkdownOptions {
        self.options
    }

    pub fn set_styles(&mut self, styles: MarkdownStyles) {
        // Recolor only: block identity, text and source ranges stay unchanged
        // (`Markdown.ts:2280-2370` rerenderBlocks path).
        self.styles = styles;
    }

    #[must_use]
    pub fn styles(&self) -> &MarkdownStyles {
        &self.styles
    }

    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    #[must_use]
    pub fn blocks(&self) -> &[MarkdownBlock] {
        &self.blocks
    }

    #[must_use]
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    /// Number of tokens whose exact `raw` prefix matched the previous parse
    /// (`markdown-parser.ts:31-46`).
    #[must_use]
    pub fn reused_tokens(&self) -> usize {
        self.reused_tokens
    }

    /// Renderer identity epoch; changes only when the effective fence handler
    /// map changes (`markdown.ts:10-17`).
    #[must_use]
    pub fn renderer_epoch(&self) -> u64 {
        self.renderer_epoch
    }

    /// Bind the effective plugin registry. A different effective map clears and
    /// rebuilds block state, exactly like `set renderNode`
    /// (`Markdown.ts:503-511`); an unchanged map keeps every block.
    pub fn set_fence_handlers(&mut self, registry: &MarkdownPluginRegistry) {
        let next: Vec<(String, String)> = registry.effective().to_vec();
        if next == self.fence_handlers {
            return;
        }
        self.fence_handlers = next;
        self.renderer_epoch = self.renderer_epoch.wrapping_add(1);
        self.rebuild();
    }

    pub fn set_conceal(&mut self, conceal: bool) {
        if self.options.conceal != conceal {
            self.options.conceal = conceal;
            // Concealment changes rendered text only; block identity and source
            // ranges are preserved so selection/scroll anchors survive
            // (`Markdown.ts:466-472`).
            self.rebuild();
        }
    }

    pub fn set_streaming(&mut self, streaming: bool) {
        if self.options.streaming != streaming {
            self.options.streaming = streaming;
            self.rebuild();
        }
    }

    /// Mark the stream complete; equivalent to `streaming={false}` upstream and
    /// safe in either order relative to the final content update.
    pub fn finish(&mut self) {
        self.set_streaming(false);
    }

    /// Replace the whole content (authoritative resync).
    pub fn set_content(&mut self, content: &str) {
        let bounded = clip_utf8_prefix(content, MAX_FLOW_BYTES);
        self.truncated = content.len() > bounded.len();
        if self.content == bounded {
            return;
        }
        self.content.clear();
        self.content.push_str(bounded);
        self.rebuild();
    }

    /// Append one decoded chunk. Empty chunks are no-ops that preserve existing
    /// text (`Markdown.ts:430-437`).
    pub fn push_chunk(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }
        self.append_text(chunk);
    }

    /// Append one transport chunk that may end inside a UTF-8 scalar. The
    /// incomplete tail is buffered and prepended to the next chunk, so split
    /// Unicode cannot be lost or replaced.
    pub fn push_bytes(&mut self, chunk: &[u8]) {
        if chunk.is_empty() {
            return;
        }
        let mut buffer = std::mem::take(&mut self.pending_utf8);
        buffer.extend_from_slice(chunk);
        let keep = incomplete_tail(&buffer);
        let split = buffer.len() - keep;
        let tail = buffer.split_off(split);
        self.pending_utf8 = tail;
        let decoded = String::from_utf8_lossy(&buffer).into_owned();
        self.append_text(&decoded);
    }

    /// Flush a buffered incomplete scalar as the replacement character; call
    /// once when the transport closes.
    pub fn flush_pending(&mut self) {
        if self.pending_utf8.is_empty() {
            return;
        }
        let tail = std::mem::take(&mut self.pending_utf8);
        self.append_text(&String::from_utf8_lossy(&tail));
    }

    #[must_use]
    pub fn anchor(&self, block: usize) -> Option<(usize, usize)> {
        self.blocks.get(block).map(|entry| (entry.source_start, entry.source_end))
    }

    /// Render the whole flow at `width` cells, honouring `marginTop`, block
    /// padding and table geometry.
    #[must_use]
    pub fn render(&self, width: usize) -> Vec<FlowRow> {
        let mut rows = Vec::new();
        for block in &self.blocks {
            for _ in 0..block.margin_top {
                rows.push(FlowRow::blank(block.id));
            }
            match &block.body {
                BlockBody::Inline(lines) => {
                    for line in lines {
                        rows.push(FlowRow { block: block.id, spans: inline_spans(line) });
                    }
                }
                BlockBody::Code { lines, .. } => {
                    for line in lines {
                        rows.push(FlowRow {
                            block: block.id,
                            spans: if line.is_empty() {
                                Vec::new()
                            } else {
                                vec![RowSpan::new(line.clone(), MarkdownRole::CodeBlock)]
                            },
                        });
                    }
                }
                BlockBody::List(items) => {
                    self.render_list(block.id, items, 0, &mut rows);
                }
                BlockBody::Quote(lines) => {
                    for line in lines {
                        let mut spans = vec![
                            RowSpan::new("│", MarkdownRole::Rule),
                            RowSpan::new(" ", MarkdownRole::BlockQuote),
                        ];
                        spans.extend(inline_spans(line));
                        rows.push(FlowRow { block: block.id, spans });
                    }
                }
                BlockBody::Rule => {
                    if width > 0 {
                        rows.push(FlowRow {
                            block: block.id,
                            spans: vec![RowSpan::new("─".repeat(width), MarkdownRole::Rule)],
                        });
                    } else {
                        rows.push(FlowRow::blank(block.id));
                    }
                }
                BlockBody::Table(table) => {
                    self.render_table(block.id, table, width, &mut rows);
                }
            }
        }
        rows
    }

    fn render_list(&self, block: usize, items: &[ListItem], indent: usize, rows: &mut Vec<FlowRow>) {
        for item in items {
            let marker = format!("{:>width$}", item.marker, width = item.marker_width);
            let mut first = vec![
                RowSpan::new(" ".repeat(indent), MarkdownRole::Text),
                RowSpan::new(format!("{marker} "), MarkdownRole::ListMarker),
            ];
            if let Some(line) = item.lines.first() {
                first.extend(inline_spans(line));
            }
            rows.push(FlowRow { block, spans: first });
            let hanging = indent.saturating_add(item.marker_width).saturating_add(1);
            for line in item.lines.iter().skip(1) {
                let mut spans = vec![RowSpan::new(" ".repeat(hanging), MarkdownRole::Text)];
                spans.extend(inline_spans(line));
                rows.push(FlowRow { block, spans });
            }
            self.render_list(block, &item.children, hanging, rows);
            if item.loose_after {
                rows.push(FlowRow::blank(block));
            }
        }
    }

    fn render_table(&self, block: usize, table: &Table, width: usize, rows: &mut Vec<FlowRow>) {
        let options = self.options.table;
        let columns = table.header.len();
        if columns == 0 {
            return;
        }
        let mut intrinsic = vec![1usize; columns];
        for (index, cell) in table.header.iter().enumerate().take(columns) {
            intrinsic[index] = intrinsic[index].max(if cell.text.is_empty() { 1 } else { cell.cell_width() });
        }
        for row in &table.rows {
            for (index, cell) in row.iter().enumerate().take(columns) {
                intrinsic[index] = intrinsic[index].max(if cell.text.is_empty() { 1 } else { cell.cell_width() });
            }
        }
        let padding = options.cell_padding_x.saturating_mul(2);
        let mut widths: Vec<usize> = intrinsic
            .iter()
            .map(|content| content.saturating_add(padding).max(1))
            .collect();

        let borders = options.style == TableStyle::Grid && options.borders;
        let separators = columns.saturating_sub(1);
        let fixed = if borders { columns + 1 } else { separators * 2 };
        let target = width.saturating_sub(fixed);
        let current: usize = widths.iter().sum();
        if borders && options.width_mode == TableWidthMode::Full && current < target {
            // `expandColumnWidths` (`TextTable.ts:740-763`).
            let extra = target - current;
            let shared = extra / columns;
            let remainder = extra % columns;
            for (index, value) in widths.iter_mut().enumerate() {
                *value += shared + usize::from(index < remainder);
            }
        } else if current > target && options.wrap {
            // `fitColumnWidthsProportional` (`text-table-width.ts:15-97`).
            widths = fit_proportional(&widths, target, padding + 1);
        }

        if !borders {
            // Borderless `columns` style: two-cell column gap
            // (`Markdown.ts:1555-1560`, `TextTable.ts:924-931`).
            for row in std::iter::once(&table.header).chain(table.rows.iter()) {
                for line in self.table_row_lines(row, &widths, options, row_is_header(row, &table.header)) {
                    rows.push(FlowRow { block, spans: line });
                }
            }
            return;
        }

        rows.push(FlowRow {
            block,
            spans: vec![RowSpan::new(grid_border(&widths, BorderRow::Top), MarkdownRole::Rule)],
        });
        for line in self.table_row_lines(&table.header, &widths, options, true) {
            rows.push(FlowRow { block, spans: line });
        }
        if !table.rows.is_empty() {
            rows.push(FlowRow {
                block,
                spans: vec![RowSpan::new(grid_border(&widths, BorderRow::Middle), MarkdownRole::Rule)],
            });
        }
        for (position, row) in table.rows.iter().enumerate() {
            for line in self.table_row_lines(row, &widths, options, false) {
                rows.push(FlowRow { block, spans: line });
            }
            if position + 1 < table.rows.len() {
                rows.push(FlowRow {
                    block,
                    spans: vec![RowSpan::new(grid_border(&widths, BorderRow::Middle), MarkdownRole::Rule)],
                });
            }
        }
        rows.push(FlowRow {
            block,
            spans: vec![RowSpan::new(grid_border(&widths, BorderRow::Bottom), MarkdownRole::Rule)],
        });
    }

    fn table_row_lines(
        &self,
        row: &[InlineLine],
        widths: &[usize],
        options: TableOptions,
        header: bool,
    ) -> Vec<Vec<RowSpan>> {
        let padding = options.cell_padding_x;
        let role = if header { MarkdownRole::Heading } else { MarkdownRole::Text };
        let wrapped: Vec<Vec<Vec<RowSpan>>> = widths
            .iter()
            .enumerate()
            .map(|(index, width)| {
                let content_width = width.saturating_sub(padding.saturating_mul(2)).max(1);
                let mut chunks: Vec<RowSpan> = Vec::new();
                if let Some(line) = row.get(index) {
                    for span in inline_spans(line) {
                        chunks.push(if header { RowSpan::new(span.text, MarkdownRole::Heading) } else { span });
                    }
                }
                if chunks.is_empty() {
                    // Empty cell => one space (`Markdown.ts:1430-1437`).
                    chunks.push(RowSpan::new(" ", role));
                }
                let mut lines = if options.wrap { wrap_spans(&chunks, content_width) } else { vec![chunks] };
                for line in &mut lines {
                    let used = line_cells(line);
                    line.push(RowSpan::new(" ".repeat(content_width.saturating_sub(used)), role));
                }
                lines
            })
            .collect();
        let height = wrapped.iter().map(Vec::len).max().unwrap_or(1).max(1);
        let bordered = options.style == TableStyle::Grid && options.borders;
        let mut out = Vec::with_capacity(height);
        for line_index in 0..height {
            let mut spans = Vec::new();
            if bordered {
                spans.push(RowSpan::new("│", MarkdownRole::Rule));
            }
            for (cell_index, cell) in wrapped.iter().enumerate() {
                if cell_index > 0 {
                    spans.push(RowSpan::new(if bordered { "│" } else { "  " }, MarkdownRole::Rule));
                }
                spans.push(RowSpan::new(" ".repeat(padding), MarkdownRole::Text));
                if let Some(line) = cell.get(line_index) {
                    spans.extend(line.iter().cloned());
                } else {
                    // Shorter cell: pad the content area only; the surrounding
                    // `padding` spans are pushed unconditionally below.
                    let width = widths.get(cell_index).copied().unwrap_or(1);
                    spans.push(RowSpan::new(
                        " ".repeat(width.saturating_sub(padding.saturating_mul(2))),
                        MarkdownRole::Text,
                    ));
                }
                spans.push(RowSpan::new(" ".repeat(padding), MarkdownRole::Text));
            }
            if bordered {
                spans.push(RowSpan::new("│", MarkdownRole::Rule));
            }
            out.push(spans);
        }
        out
    }

    fn append_text(&mut self, chunk: &str) {
        let room = MAX_FLOW_BYTES.saturating_sub(self.content.len());
        if room == 0 {
            self.truncated = true;
            return;
        }
        let bounded = clip_utf8_prefix(chunk, room);
        if bounded.len() < chunk.len() {
            self.truncated = true;
        }
        self.content.push_str(bounded);
        self.rebuild();
    }

    fn rebuild(&mut self) {
        let previous: Vec<(&'static str, String)> =
            self.blocks.iter().map(|block| (block_key(&block.kind), block.source_raw.clone())).collect();
        let tokens = tokenize(&self.content);
        // Token identity is reused only for an exact raw prefix of the previous
        // parse (`markdown-parser.ts:31-46`), and the last
        // `TRAILING_UNSTABLE_TOKENS` matches are re-lexed while streaming so a
        // growing tail (`# Hello` -> `# Hello World`) is never pinned
        // (`markdown-parser.ts:47-48`).
        let prefix = reused_prefix(&self.tokens, &self.content);
        self.reused_tokens = if self.options.streaming {
            prefix.saturating_sub(TRAILING_UNSTABLE_TOKENS)
        } else {
            prefix
        };
        let mut blocks: Vec<MarkdownBlock> = Vec::new();
        let mut gap = String::new();
        let mut previous_kind: Option<RawKind> = None;
        let mut previous_raw = String::new();
        for token in &tokens {
            if token.kind == RawKind::Space {
                gap.push_str(&token.raw);
                continue;
            }
            let margin_top = match previous_kind {
                None => 0,
                Some(kind) => usize::from(top_level_margin(kind, &previous_raw, token.kind, &gap)),
            };
            if blocks.len() >= MAX_BLOCKS {
                self.truncated = true;
                break;
            }
            let preserved = previous
                .get(blocks.len())
                .is_some_and(|(kind, raw)| *kind == token_key(token.kind) && *raw == token.raw);
            blocks.push(self.build_block(blocks.len(), token, margin_top, preserved));
            previous_kind = Some(token.kind);
            previous_raw.clone_from(&token.raw);
            gap.clear();
        }
        self.tokens = tokens;
        self.blocks = blocks;
    }

    fn build_block(&self, id: usize, token: &RawToken, margin_top: usize, preserved: bool) -> MarkdownBlock {
        let conceal = self.options.conceal;
        let hyperlinks = self.options.hyperlinks;
        let (kind, body) = match token.kind {
            RawKind::Space => (BlockKind::Paragraph, BlockBody::Inline(Vec::new())),
            RawKind::Paragraph => {
                let text = strip_trailing_newlines(&token.raw);
                let line = parse_inline(text, MarkdownRole::Text, conceal, hyperlinks);
                (BlockKind::Paragraph, BlockBody::Inline(vec![line]))
            }
            RawKind::Heading => {
                let level = heading_level(&token.raw).unwrap_or(1);
                let text = heading_text(&token.raw);
                let source = strip_trailing_newlines(&token.raw);
                let display = if conceal { text } else { source };
                let base = if level == 1 { MarkdownRole::Heading1 } else { MarkdownRole::Heading };
                let line = parse_inline(display, base, conceal, hyperlinks);
                (BlockKind::Heading(level), BlockBody::Inline(vec![line]))
            }
            RawKind::Code => {
                let (lang, content) = fence_parts(&token.raw);
                let lines: Vec<String> = content.split('\n').map(str::to_owned).collect();
                (BlockKind::Code { lang: lang.clone() }, BlockBody::Code { lines, lang })
            }
            RawKind::List => {
                let (ordered, start) = list_meta(&token.raw);
                let items = parse_items(&token.raw, 0);
                (BlockKind::List { ordered, start }, BlockBody::List(items))
            }
            RawKind::Quote => {
                let mut lines = Vec::new();
                for span in line_spans(&token.raw) {
                    let text = &token.raw[span.start..span.text_end];
                    let stripped = strip_quote_prefix(text);
                    lines.push(parse_inline(stripped, MarkdownRole::BlockQuote, conceal, hyperlinks));
                }
                if lines.is_empty() {
                    lines.push(InlineLine::plain(" "));
                }
                (BlockKind::BlockQuote, BlockBody::Quote(lines))
            }
            RawKind::Rule => (BlockKind::Rule, BlockBody::Rule),
            RawKind::Table => {
                let table = parse_table(&token.raw, conceal, hyperlinks);
                if table.rows.is_empty() {
                    // Header-only tables have no content cache and fall back to
                    // raw markdown (`Markdown.ts:1467-1470`, `1636-1655`).
                    let source = strip_trailing_newlines(&token.raw);
                    let line = parse_inline(source, MarkdownRole::Text, conceal, hyperlinks);
                    (BlockKind::Paragraph, BlockBody::Inline(vec![line]))
                } else {
                    (BlockKind::Table, BlockBody::Table(table))
                }
            }
        };
        let fence_handler = match &body {
            BlockBody::Code { lang, .. } => lang.as_deref().and_then(|language| self.handler_for(language)),
            _ => None,
        };
        MarkdownBlock {
            id,
            kind,
            source_start: token.start,
            source_end: token.start + token.raw.len(),
            source_raw: token.raw.clone(),
            margin_top,
            preserved,
            fence_handler,
            body,
        }
    }

    fn handler_for(&self, language: &str) -> Option<String> {
        self.fence_handlers
            .iter()
            .find(|(key, _)| key == language)
            .map(|(_, handler)| handler.clone())
    }
}

fn block_key(kind: &BlockKind) -> &'static str {
    match kind {
        BlockKind::Paragraph => "paragraph",
        BlockKind::Heading(_) => "heading",
        BlockKind::Code { .. } => "code",
        BlockKind::List { .. } => "list",
        BlockKind::BlockQuote => "blockquote",
        BlockKind::Rule => "hr",
        BlockKind::Table => "table",
    }
}

fn token_key(kind: RawKind) -> &'static str {
    match kind {
        RawKind::Space => "space",
        RawKind::Paragraph => "paragraph",
        RawKind::Heading => "heading",
        RawKind::Code => "code",
        RawKind::List => "list",
        RawKind::Quote => "blockquote",
        RawKind::Rule => "hr",
        RawKind::Table => "table",
    }
}

/// `shouldAddTopLevelMargin` (`Markdown.ts:1366-1374`) and
/// `TRAILING_MARKDOWN_BLOCK_BREAKS_RE` (`Markdown.ts:229`).
fn top_level_margin(prev: RawKind, prev_raw: &str, current: RawKind, gap: &str) -> bool {
    if is_separated(prev) || is_separated(current) {
        return true;
    }
    prev == RawKind::Paragraph && current == RawKind::Paragraph && ends_with_blank_line(prev_raw, gap)
}

fn is_separated(kind: RawKind) -> bool {
    matches!(
        kind,
        RawKind::Heading | RawKind::List | RawKind::Code | RawKind::Table | RawKind::Quote | RawKind::Rule
    )
}

fn ends_with_blank_line(raw: &str, gap: &str) -> bool {
    let combined = format!("{raw}{gap}");
    let trimmed = combined.trim_end_matches(['\r', '\n']);
    combined[trimmed.len()..].matches('\n').count() >= 2
}

fn strip_trailing_newlines(text: &str) -> &str {
    text.trim_end_matches(['\r', '\n'])
}

/// Token prefix reuse (`markdown-parser.ts:31-46`): exact `raw` bytes from the
/// start of the new content keep their identity.
fn reused_prefix(tokens: &[RawToken], content: &str) -> usize {
    let mut offset = 0usize;
    let mut reuse = 0usize;
    for token in tokens {
        if offset + token.raw.len() <= content.len() && content[offset..].starts_with(&token.raw) {
            reuse += 1;
            offset += token.raw.len();
        } else {
            break;
        }
    }
    reuse
}

fn line_spans(text: &str) -> Vec<LineSpan> {
    let mut spans = Vec::new();
    let bytes = text.as_bytes();
    let mut start = 0usize;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            let mut text_end = index;
            if text_end > start && bytes[text_end - 1] == b'\r' {
                text_end -= 1;
            }
            spans.push(LineSpan { start, end: index + 1, text_end });
            start = index + 1;
        }
    }
    if start < text.len() {
        spans.push(LineSpan { start, end: text.len(), text_end: text.len() });
    }
    spans
}

fn line_text<'a>(raw: &'a str, span: &LineSpan) -> &'a str {
    &raw[span.start..span.text_end]
}

fn tokenize(content: &str) -> Vec<RawToken> {
    let lines = line_spans(content);
    let mut tokens = Vec::new();
    let mut index = 0usize;
    while index < lines.len() {
        let span = lines[index];
        let text = line_text(content, &span);
        if text.trim().is_empty() {
            let begin = span.start;
            let mut last = index;
            while last + 1 < lines.len() && line_text(content, &lines[last + 1]).trim().is_empty() {
                last += 1;
            }
            tokens.push(RawToken {
                kind: RawKind::Space,
                raw: content[begin..lines[last].end].to_owned(),
                start: begin,
            });
            index = last + 1;
            continue;
        }
        if let Some((marker, count)) = fence_open(text) {
            let begin = span.start;
            let mut last = index;
            let mut closed = false;
            for probe in index + 1..lines.len() {
                if fence_close(line_text(content, &lines[probe]), marker, count) {
                    last = probe;
                    closed = true;
                    break;
                }
                last = probe;
            }
            let end = if closed { lines[last].end } else { content.len() };
            tokens.push(RawToken { kind: RawKind::Code, raw: content[begin..end].to_owned(), start: begin });
            index = last + 1;
            continue;
        }
        if heading_level(text).is_some() {
            tokens.push(RawToken {
                kind: RawKind::Heading,
                raw: content[span.start..span.end].to_owned(),
                start: span.start,
            });
            index += 1;
            continue;
        }
        if is_rule(text) {
            tokens.push(RawToken {
                kind: RawKind::Rule,
                raw: content[span.start..span.end].to_owned(),
                start: span.start,
            });
            index += 1;
            continue;
        }
        if is_quote_line(text) {
            let begin = span.start;
            let mut last = index;
            while last + 1 < lines.len() {
                let next = line_text(content, &lines[last + 1]);
                let quoted_gap = next.trim().is_empty()
                    && lines
                        .get(last + 2)
                        .is_some_and(|probe| is_quote_line(line_text(content, probe)));
                if is_quote_line(next) || quoted_gap {
                    last += 1;
                } else {
                    break;
                }
            }
            tokens.push(RawToken { kind: RawKind::Quote, raw: content[begin..lines[last].end].to_owned(), start: begin });
            index = last + 1;
            continue;
        }
        if text.contains('|') && index + 1 < lines.len() {
            let delimiter = line_text(content, &lines[index + 1]);
            if table_delimiter(delimiter).is_some() {
                let begin = span.start;
                let mut last = index + 1;
                for probe in index + 2..lines.len() {
                    let candidate = line_text(content, &lines[probe]);
                    if candidate.trim().is_empty() || !candidate.contains('|') {
                        break;
                    }
                    last = probe;
                }
                tokens.push(RawToken {
                    kind: RawKind::Table,
                    raw: content[begin..lines[last].end].to_owned(),
                    start: begin,
                });
                index = last + 1;
                continue;
            }
        }
        if let Some((_, _, _)) = list_marker(text) {
            let ordered = list_marker(text).is_some_and(|(_, marker, _)| {
                marker.chars().next().is_some_and(|ch| ch.is_ascii_digit())
            });
            let begin = span.start;
            let mut last = index;
            for probe in index + 1..lines.len() {
                let candidate = line_text(content, &lines[probe]);
                if candidate.trim().is_empty() {
                    let next = lines.get(probe + 1).map(|next| line_text(content, next));
                    match next {
                        Some(next_text)
                            if indent_of(next_text) >= 2
                                || list_marker(next_text)
                                    .is_some_and(|(_, marker, _)| is_ordered(marker) == ordered) =>
                        {
                            last = probe;
                            continue;
                        }
                        _ => break,
                    }
                }
                if indent_of(candidate) >= 2
                    || list_marker(candidate).is_some_and(|(_, marker, _)| is_ordered(marker) == ordered)
                {
                    last = probe;
                    continue;
                }
                break;
            }
            tokens.push(RawToken { kind: RawKind::List, raw: content[begin..lines[last].end].to_owned(), start: begin });
            index = last + 1;
            continue;
        }
        // Paragraph: consecutive lines until a blank line or a new block.
        let begin = span.start;
        let mut last = index;
        for probe in index + 1..lines.len() {
            let candidate = line_text(content, &lines[probe]);
            if candidate.trim().is_empty()
                || fence_open(candidate).is_some()
                || heading_level(candidate).is_some()
                || is_rule(candidate)
                || is_quote_line(candidate)
                || list_marker(candidate).is_some()
                || (candidate.contains('|')
                    && lines
                        .get(probe + 1)
                        .is_some_and(|next| table_delimiter(line_text(content, next)).is_some()))
            {
                break;
            }
            last = probe;
        }
        tokens.push(RawToken {
            kind: RawKind::Paragraph,
            raw: content[begin..lines[last].end].to_owned(),
            start: begin,
        });
        index = last + 1;
    }
    tokens
}

fn fence_open(line: &str) -> Option<(char, usize)> {
    if indent_of(line) > 3 {
        return None;
    }
    let trimmed = line.trim_start_matches(' ');
    let marker = trimmed.chars().next()?;
    if marker != '`' && marker != '~' {
        return None;
    }
    let count = trimmed.chars().take_while(|ch| *ch == marker).count();
    if count < 3 {
        return None;
    }
    Some((marker, count))
}

fn fence_close(line: &str, marker: char, minimum: usize) -> bool {
    if indent_of(line) > 3 {
        return false;
    }
    let trimmed = line.trim_start_matches(' ');
    let count = trimmed.chars().take_while(|ch| *ch == marker).count();
    count >= minimum && trimmed[count..].trim().is_empty()
}

/// Fenced code returns its info-string language and content. Upstream paints
/// `token.text`, i.e. the fence lines and info string are excluded
/// (`Markdown.ts:1171-1186`).
fn fence_parts(raw: &str) -> (Option<String>, String) {
    let spans = line_spans(raw);
    if spans.is_empty() {
        return (None, String::new());
    }
    let first = line_text(raw, &spans[0]);
    let trimmed = first.trim_start_matches(' ');
    let marker = trimmed.chars().next().unwrap_or('`');
    let count = trimmed.chars().take_while(|ch| *ch == marker).count();
    let info = trimmed[count..].trim();
    let lang = info
        .split_whitespace()
        .next()
        .map(|language| language.trim_matches(['{', '}', '.']).to_ascii_lowercase())
        .filter(|language| !language.is_empty());
    let closed = spans.len() > 1 && fence_close(line_text(raw, &spans[spans.len() - 1]), marker, count);
    let body_end = if closed { spans.len() - 1 } else { spans.len() };
    let mut content = String::new();
    for position in (1..body_end).filter(|position| *position < spans.len()) {
        if position > 1 {
            content.push('\n');
        }
        content.push_str(line_text(raw, &spans[position]));
    }
    (lang, content)
}

fn heading_level(line: &str) -> Option<u8> {
    let indent = indent_of(line);
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let level = rest.chars().take_while(|ch| *ch == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let after = &rest[level..];
    if after.is_empty() || after.starts_with(' ') || after.starts_with('\t') {
        Some(level as u8)
    } else {
        None
    }
}

fn heading_text(raw: &str) -> &str {
    let line = raw.split('\n').next().unwrap_or(raw);
    let line = line.trim_end_matches('\r');
    let indent = indent_of(line);
    let rest = &line[indent..];
    let level = rest.chars().take_while(|ch| *ch == '#').count();
    rest[level..].trim_start_matches([' ', '\t']).trim_end_matches([' ', '\t'])
}

fn is_rule(line: &str) -> bool {
    if indent_of(line) > 3 {
        return false;
    }
    let trimmed = line.trim();
    let marker = match trimmed.chars().next() {
        Some(ch @ ('-' | '*' | '_')) => ch,
        _ => return false,
    };
    let mut count = 0usize;
    for ch in trimmed.chars() {
        if ch == marker {
            count += 1;
        } else if ch != ' ' && ch != '\t' {
            return false;
        }
    }
    count >= 3
}

fn is_quote_line(line: &str) -> bool {
    indent_of(line) <= 3 && line.trim_start_matches(' ').starts_with('>')
}

fn strip_quote_prefix(line: &str) -> &str {
    let trimmed = line.trim_start_matches(' ');
    let rest = trimmed.strip_prefix('>').unwrap_or(trimmed);
    rest.strip_prefix(' ').unwrap_or(rest)
}

fn table_delimiter(line: &str) -> Option<usize> {
    let cells = split_row(line);
    if cells.is_empty() {
        return None;
    }
    for cell in &cells {
        let trimmed = cell.trim();
        let core = trimmed.trim_matches(':');
        if core.is_empty() || !core.chars().all(|ch| ch == '-') {
            return None;
        }
        if trimmed.starts_with(':') && trimmed.len() != core.len() + 1 {
            return None;
        }
    }
    Some(cells.len())
}

fn split_row(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for ch in line.trim().chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => {
                current.push(ch);
                escaped = true;
            }
            '|' => cells.push(std::mem::take(&mut current)),
            _ => current.push(ch),
        }
    }
    cells.push(current);
    if cells.first().is_some_and(|cell| cell.trim().is_empty()) {
        cells.remove(0);
    }
    if cells.last().is_some_and(|cell| cell.trim().is_empty()) {
        cells.pop();
    }
    cells
}

fn parse_table(raw: &str, conceal: bool, hyperlinks: bool) -> Table {
    let spans = line_spans(raw);
    if spans.len() < 2 {
        return Table::default();
    }
    let header_cells = split_row(line_text(raw, &spans[0]));
    let columns = header_cells.len().min(MAX_TABLE_COLUMNS);
    if columns == 0 || table_delimiter(line_text(raw, &spans[1])).is_none() {
        return Table::default();
    }
    let header = header_cells
        .iter()
        .take(columns)
        .map(|cell| parse_inline(cell.trim(), MarkdownRole::Text, conceal, hyperlinks))
        .collect();
    let mut rows = Vec::new();
    for span in spans.iter().skip(2).take(MAX_TABLE_ROWS) {
        let text = line_text(raw, span);
        if text.trim().is_empty() {
            break;
        }
        let cells = split_row(text);
        let mut row = Vec::with_capacity(columns);
        for index in 0..columns {
            match cells.get(index) {
                Some(cell) => row.push(parse_inline(cell.trim(), MarkdownRole::Text, conceal, hyperlinks)),
                // Missing cells normalize to a single space
                // (`Markdown.ts:1430-1437`).
                None => row.push(InlineLine::plain(" ")),
            }
        }
        rows.push(row);
    }
    Table { header, rows }
}

fn list_marker(line: &str) -> Option<(usize, &str, &str)> {
    let indent = indent_of(line);
    let rest = &line[indent..];
    let first = rest.chars().next()?;
    if matches!(first, '-' | '*' | '+') {
        let after = &rest[1..];
        if after.starts_with(' ') || after.starts_with('\t') {
            return Some((indent, &rest[..1], after.trim_start_matches([' ', '\t'])));
        }
        return None;
    }
    if first.is_ascii_digit() {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if digits > 9 {
            return None;
        }
        let after = &rest[digits..];
        let separator = after.chars().next()?;
        if (separator == '.' || separator == ')')
            && (after[1..].starts_with(' ') || after[1..].starts_with('\t'))
        {
            return Some((indent, &rest[..digits + 1], after[1..].trim_start_matches([' ', '\t'])));
        }
    }
    None
}

fn is_ordered(marker: &str) -> bool {
    marker.chars().next().is_some_and(|ch| ch.is_ascii_digit())
}

fn list_meta(raw: &str) -> (bool, usize) {
    for span in line_spans(raw) {
        let text = line_text(raw, &span);
        if let Some((_, marker, _)) = list_marker(text) {
            let ordered = is_ordered(marker);
            let start = marker.trim_end_matches(['.', ')']).parse::<usize>().unwrap_or(1);
            return (ordered, if ordered { start.max(1) } else { 1 });
        }
    }
    (false, 1)
}

fn parse_items(raw: &str, base_indent: usize) -> Vec<ListItem> {
    let spans = line_spans(raw);
    parse_items_inner(raw, &spans, base_indent)
}

fn parse_items_inner(raw: &str, spans: &[LineSpan], base_indent: usize) -> Vec<ListItem> {
    let lines: Vec<&str> = spans.iter().map(|span| line_text(raw, span)).collect();
    let mut items: Vec<ListItem> = Vec::new();
    let mut index = 0usize;
    let mut ordered_start: Option<usize> = None;
    while index < lines.len() {
        let text = lines[index];
        if text.trim().is_empty() {
            index += 1;
            continue;
        }
        let Some((marker_indent, marker, content)) = list_marker(text) else {
            index += 1;
            continue;
        };
        if marker_indent != base_indent {
            index += 1;
            continue;
        }
        let marker_text = match marker.trim_end_matches(['.', ')']).parse::<usize>() {
            Ok(number) => {
                let start = *ordered_start.get_or_insert(number);
                format!("{}.", start.saturating_add(items.len()))
            }
            Err(_) => "-".to_owned(),
        };
        let mut lines_out = vec![parse_inline(content.trim_end(), MarkdownRole::Text, false, false)];
        let mut children: Vec<ListItem> = Vec::new();
        let mut loose_after = false;
        index += 1;
        while index < lines.len() {
            let next = lines[index];
            if next.trim().is_empty() {
                match lines.get(index + 1) {
                    Some(following) if indent_of(following) > marker_indent || list_marker(following).is_some() => {
                        loose_after = true;
                        index += 1;
                        continue;
                    }
                    _ => break,
                }
            }
            let next_indent = indent_of(next);
            if next_indent > marker_indent {
                if list_marker(next).is_some() {
                    let child_start = index;
                    let mut child_end = index;
                    while child_end + 1 < lines.len() {
                        let probe = lines[child_end + 1];
                        if probe.trim().is_empty() || indent_of(probe) >= next_indent {
                            child_end += 1;
                        } else {
                            break;
                        }
                    }
                    children.extend(parse_items_inner(raw, &spans[child_start..=child_end], next_indent));
                    index = child_end + 1;
                    continue;
                }
                lines_out.push(parse_inline(next.trim(), MarkdownRole::Text, false, false));
                index += 1;
                continue;
            }
            break;
        }
        items.push(ListItem { marker: marker_text, marker_width: 1, lines: lines_out, children, loose_after });
    }
    let widest = items.iter().map(|item| item.marker.len()).max().unwrap_or(1).max(1);
    for item in &mut items {
        item.marker_width = widest;
    }
    items
}

fn indent_of(line: &str) -> usize {
    line.chars().take_while(|ch| *ch == ' ').count()
}

fn clip_utf8_prefix(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Number of trailing bytes forming an incomplete UTF-8 scalar.
fn incomplete_tail(buffer: &[u8]) -> usize {
    let limit = buffer.len().min(3);
    for back in 1..=limit {
        let split = buffer.len() - back;
        let byte = buffer[split];
        if byte & 0b1100_0000 == 0b1000_0000 {
            continue; // continuation byte: the lead byte is earlier
        }
        let needed = if byte < 0x80 {
            1
        } else if byte >> 5 == 0b110 {
            2
        } else if byte >> 4 == 0b1110 {
            3
        } else if byte >> 3 == 0b11110 {
            4
        } else {
            1
        };
        return if split + needed > buffer.len() { buffer.len() - split } else { 0 };
    }
    0
}

/// Display cells for `text` using the shared cluster visitor.
pub fn cells(text: &str) -> usize {
    visit_unpadded_cells(text, text.len().max(1), |_, _| {})
}

fn clusters(text: &str) -> Vec<(String, usize)> {
    let mut output = Vec::new();
    visit_unpadded_cells(text, text.len().max(1), |glyph, width| output.push((glyph.to_owned(), width)));
    output
}

fn line_cells(spans: &[RowSpan]) -> usize {
    spans.iter().map(|span| cells(&span.text)).sum()
}

fn inline_spans(line: &InlineLine) -> Vec<RowSpan> {
    line.spans
        .iter()
        .filter(|span| span.end > span.start && span.end <= line.text.len())
        .map(|span| RowSpan::new(line.text[span.start..span.end].to_owned(), span.role))
        .collect()
}

fn row_is_header(row: &[InlineLine], header: &[InlineLine]) -> bool {
    std::ptr::eq(row, header)
}

struct InlineBuilder {
    text: String,
    spans: Vec<InlineSpan>,
    conceal: bool,
    hyperlinks: bool,
}

impl InlineBuilder {
    fn new(conceal: bool, hyperlinks: bool) -> Self {
        Self { text: String::new(), spans: Vec::new(), conceal, hyperlinks }
    }

    fn emit(&mut self, value: &str, role: MarkdownRole) {
        if value.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(value);
        let end = self.text.len();
        if let Some(last) = self.spans.last_mut() {
            if last.role == role && last.end == start {
                last.end = end;
                return;
            }
        }
        if self.spans.len() < MAX_INLINE_SPANS {
            self.spans.push(InlineSpan { start, end, role });
        }
    }

    fn parse(&mut self, source: &str, role: MarkdownRole) {
        let bytes = source.as_bytes();
        let mut index = 0usize;
        let mut plain_start = 0usize;
        while index < bytes.len() {
            if !is_special(source, index) {
                index += 1;
                continue;
            }
            if plain_start < index {
                self.emit(&source[plain_start..index], role);
            }
            match self.parse_special(source, index, role) {
                Some(next) => {
                    index = next;
                    plain_start = next;
                }
                None => {
                    let next = next_char(source, index);
                    self.emit(&source[index..next], role);
                    index = next;
                    plain_start = next;
                }
            }
        }
        if plain_start < source.len() {
            self.emit(&source[plain_start..], role);
        }
    }

    fn parse_special(&mut self, source: &str, index: usize, role: MarkdownRole) -> Option<usize> {
        let bytes = source.as_bytes();
        match bytes[index] {
            b'\\' => {
                let next = next_char(source, index);
                if next < source.len() {
                    let after = next_char(source, next);
                    self.emit(&source[next..after], role);
                    Some(after)
                } else {
                    self.emit("\\", role);
                    Some(next)
                }
            }
            b'`' => {
                let ticks = run_len(source, index, b'`');
                let content_start = index + ticks;
                let close = find_ticks(source, content_start, ticks)?;
                let content = strip_codespan_padding(&source[content_start..close]);
                self.emit(content, MarkdownRole::InlineCode);
                Some(close + ticks)
            }
            b'*' | b'_' => {
                let marker = bytes[index];
                if marker == b'_' && intraword(source, index) {
                    return None;
                }
                let count = run_len(source, index, marker);
                let width = if count >= 2 { 2 } else { 1 };
                let inner_role = if width == 2 { MarkdownRole::Strong } else { MarkdownRole::Emphasis };
                let content_start = index + width;
                if !content_starts_flanked(source, content_start) {
                    return None;
                }
                let close = find_closer(source, content_start, marker, width)?;
                if self.conceal {
                    self.parse(&source[content_start..close], inner_role);
                } else {
                    let delimiter: String = std::iter::repeat_n(char::from(marker), width).collect();
                    self.emit(&delimiter, inner_role);
                    self.parse(&source[content_start..close], inner_role);
                    self.emit(&delimiter, inner_role);
                }
                Some(close + width)
            }
            b'~' if run_len(source, index, b'~') >= 2 => {
                let content_start = index + 2;
                if !content_starts_flanked(source, content_start) {
                    return None;
                }
                let close = find_closer(source, content_start, b'~', 2)?;
                if self.conceal {
                    self.parse(&source[content_start..close], MarkdownRole::Strikethrough);
                } else {
                    self.emit("~~", MarkdownRole::Strikethrough);
                    self.parse(&source[content_start..close], MarkdownRole::Strikethrough);
                    self.emit("~~", MarkdownRole::Strikethrough);
                }
                Some(close + 2)
            }
            b'!' if bytes.get(index + 1) == Some(&b'[') => {
                let (label, target, next) = parse_link_target(source, index + 2)?;
                if self.conceal {
                    let alt = if label.is_empty() { "image" } else { label };
                    self.emit(alt, MarkdownRole::LinkLabel);
                } else {
                    self.emit("![", MarkdownRole::Link);
                    self.emit(label, MarkdownRole::LinkLabel);
                    self.emit("](", MarkdownRole::Link);
                    self.emit(target, MarkdownRole::LinkUrl);
                    self.emit(")", MarkdownRole::Link);
                }
                Some(next)
            }
            b'[' => {
                let (label, target, next) = parse_link_target(source, index + 1)?;
                if self.conceal {
                    self.parse(label, MarkdownRole::LinkLabel);
                    if !self.hyperlinks && !label.is_empty() && label != target {
                        // `Markdown.ts:666-680`: non-hyperlink terminals keep the
                        // target in parentheses.
                        self.emit(" (", MarkdownRole::Link);
                        self.emit(target, MarkdownRole::LinkUrl);
                        self.emit(")", MarkdownRole::Link);
                    }
                } else {
                    self.emit("[", MarkdownRole::Link);
                    self.parse(label, MarkdownRole::LinkLabel);
                    self.emit("](", MarkdownRole::Link);
                    self.emit(target, MarkdownRole::LinkUrl);
                    self.emit(")", MarkdownRole::Link);
                }
                Some(next)
            }
            _ => None,
        }
    }
}

fn is_special(source: &str, index: usize) -> bool {
    match source.as_bytes()[index] {
        b'\\' | b'`' | b'*' | b'_' | b'~' | b'[' => true,
        b'!' => source.as_bytes().get(index + 1) == Some(&b'['),
        _ => false,
    }
}

fn next_char(text: &str, index: usize) -> usize {
    text[index..].chars().next().map_or(text.len(), |ch| index + ch.len_utf8())
}

fn run_len(text: &str, index: usize, marker: u8) -> usize {
    text.as_bytes()[index..].iter().take_while(|byte| **byte == marker).count()
}

fn content_starts_flanked(text: &str, index: usize) -> bool {
    match text[index..].chars().next() {
        Some(ch) => !ch.is_whitespace(),
        None => false,
    }
}

/// `_` inside a word is literal, matching GFM intraword rules.
fn intraword(text: &str, index: usize) -> bool {
    text[..index].chars().next_back().is_some_and(|ch| ch.is_alphanumeric())
}

fn find_ticks(text: &str, start: usize, ticks: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = start;
    while index < bytes.len() {
        if bytes[index] == b'`' {
            let run = run_len(text, index, b'`');
            if run == ticks {
                return Some(index);
            }
            index += run;
            continue;
        }
        index += 1;
    }
    None
}

fn find_closer(text: &str, start: usize, marker: u8, width: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = start;
    while index < bytes.len() {
        if bytes[index] == marker {
            let run = run_len(text, index, marker);
            if run >= width {
                let right_flanking = text[..index].chars().next_back().is_some_and(|ch| !ch.is_whitespace());
                let word_boundary = marker != b'_'
                    || !text[index + width..].chars().next().is_some_and(|ch| ch.is_alphanumeric());
                if right_flanking && word_boundary {
                    return Some(index);
                }
            }
            index += run;
            continue;
        }
        index = next_char(text, index);
    }
    None
}

fn strip_codespan_padding(content: &str) -> &str {
    if content.len() >= 2 && content.starts_with(' ') && content.ends_with(' ') && !content.trim().is_empty() {
        &content[1..content.len() - 1]
    } else {
        content
    }
}

fn parse_link_target(source: &str, label_start: usize) -> Option<(&str, &str, usize)> {
    let bytes = source.as_bytes();
    let mut index = label_start;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index = next_char(source, index),
            b']' if bytes.get(index + 1) == Some(&b'(') => {
                let label = &source[label_start..index];
                let target_start = index + 2;
                let mut probe = target_start;
                while probe < bytes.len() {
                    match bytes[probe] {
                        b'\\' => probe = next_char(source, probe),
                        b')' => return Some((label, &source[target_start..probe], probe + 1)),
                        b'\n' => return None,
                        _ => {}
                    }
                    probe += 1;
                }
                return None;
            }
            _ => {}
        }
        index = next_char(source, index);
    }
    None
}

fn parse_inline(source: &str, base: MarkdownRole, conceal: bool, hyperlinks: bool) -> InlineLine {
    let mut builder = InlineBuilder::new(conceal, hyperlinks);
    builder.parse(source, base);
    InlineLine { text: builder.text, spans: builder.spans }
}

#[derive(Clone, Copy)]
enum BorderRow {
    Top,
    Middle,
    Bottom,
}

fn grid_border(widths: &[usize], row: BorderRow) -> String {
    let (left, join, right) = match row {
        BorderRow::Top => ('┌', '┬', '┐'),
        BorderRow::Middle => ('├', '┼', '┤'),
        BorderRow::Bottom => ('└', '┴', '┘'),
    };
    let mut output = String::new();
    output.push(left);
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            output.push(join);
        }
        output.push_str(&"─".repeat(*width));
    }
    output.push(right);
    output
}

/// Greedy word wrap over styled clusters, hard-breaking words wider than the
/// column (`wrapMode: "word"`, `Markdown.ts:1560-1566`).
fn wrap_spans(chunks: &[RowSpan], width: usize) -> Vec<Vec<RowSpan>> {
    let width = width.max(1);
    let mut units: Vec<(String, usize, MarkdownRole)> = Vec::new();
    for chunk in chunks {
        for (glyph, cell_width) in clusters(&chunk.text) {
            units.push((glyph, cell_width, chunk.role));
        }
    }
    if units.is_empty() {
        return vec![Vec::new()];
    }
    let mut lines = Vec::new();
    let mut start = 0usize;
    while start < units.len() {
        let mut used = 0usize;
        let mut index = start;
        let mut last_space: Option<usize> = None;
        while index < units.len() {
            let cells = units[index].1;
            if used + cells > width && index > start {
                break;
            }
            if units[index].0 == " " {
                last_space = Some(index);
            }
            used += cells;
            index += 1;
        }
        let mut end = index;
        let mut next = index;
        if index < units.len() {
            if let Some(space) = last_space {
                if space > start {
                    end = space;
                    next = space + 1;
                }
            }
        }
        let mut trimmed = end;
        while trimmed > start && units[trimmed - 1].0 == " " {
            trimmed -= 1;
        }
        lines.push(units_to_spans(&units[start..trimmed]));
        if next <= start {
            next = start + 1;
        }
        start = next;
        while start < units.len() && units[start].0 == " " {
            start += 1;
        }
    }
    if lines.is_empty() {
        lines.push(Vec::new());
    }
    lines
}

fn units_to_spans(units: &[(String, usize, MarkdownRole)]) -> Vec<RowSpan> {
    let mut spans: Vec<RowSpan> = Vec::new();
    for (glyph, _, role) in units {
        if let Some(last) = spans.last_mut() {
            if last.role == *role {
                last.text.push_str(glyph);
                continue;
            }
        }
        spans.push(RowSpan::new(glyph.clone(), *role));
    }
    spans
}

/// Upstream proportional column fit (`text-table-width.ts:15-97`).
fn fit_proportional(widths: &[usize], target: usize, min_width: usize) -> Vec<usize> {
    let count = widths.len();
    if count == 0 {
        return Vec::new();
    }
    let base: Vec<usize> = widths.iter().map(|width| (*width).max(min_width)).collect();
    let capacity: Vec<usize> = base.iter().map(|width| width - min_width).collect();
    let total_capacity: usize = capacity.iter().sum();
    let available = target.saturating_sub(min_width * count).min(total_capacity);
    if available == 0 {
        return vec![min_width; count];
    }
    if available == total_capacity {
        return base;
    }
    if capacity.windows(2).all(|pair| pair[0] == pair[1]) {
        let shared = available / count;
        let remainder = available % count;
        return (0..count)
            .map(|index| min_width + shared + usize::from(index < remainder))
            .collect();
    }
    let mut growth = vec![0usize; count];
    let mut order: Vec<usize> = (0..count).filter(|index| capacity[*index] > 0).collect();
    order.sort_by(|left, right| {
        (capacity[*left] as f64)
            .sqrt()
            .partial_cmp(&(capacity[*right] as f64).sqrt())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut remaining = available as f64;
    let mut weight: f64 = order.iter().map(|index| (capacity[*index] as f64).sqrt()).sum();
    for index in &order {
        let column_weight = (capacity[*index] as f64).sqrt();
        if weight <= 0.0 || remaining / weight <= column_weight {
            break;
        }
        growth[*index] = capacity[*index];
        remaining -= capacity[*index] as f64;
        weight -= column_weight;
    }
    if weight > 0.0 {
        let level = remaining / weight;
        for index in &order {
            if growth[*index] == capacity[*index] {
                continue;
            }
            let column_weight = (capacity[*index] as f64).sqrt();
            growth[*index] = capacity[*index].min((level * column_weight).floor() as usize);
        }
    }
    let mut allocated: usize = growth.iter().sum();
    while allocated > available {
        let mut worst: Option<usize> = None;
        for index in 0..count {
            if growth[index] == 0 {
                continue;
            }
            worst = Some(match worst {
                None => index,
                Some(current) => {
                    if priority_greater(growth[index], capacity[index], growth[current], capacity[current]) {
                        index
                    } else {
                        current
                    }
                }
            });
        }
        let Some(worst) = worst else { break };
        growth[worst] -= 1;
        allocated -= 1;
    }
    while allocated < available {
        let mut best: Option<usize> = None;
        for index in 0..count {
            if growth[index] >= capacity[index] {
                continue;
            }
            best = Some(match best {
                None => index,
                Some(current) => {
                    if priority_greater(growth[current] + 1, capacity[current], growth[index] + 1, capacity[index]) {
                        index
                    } else {
                        current
                    }
                }
            });
        }
        let Some(best) = best else { break };
        growth[best] += 1;
        allocated += 1;
    }
    growth.iter().map(|value| value + min_width).collect()
}

fn priority_greater(growth: usize, capacity: usize, other_growth: usize, other_capacity: usize) -> bool {
    // growth^2 * otherCapacity > otherGrowth^2 * capacity
    let left = (growth as u128) * (growth as u128) * (other_capacity as u128);
    let right = (other_growth as u128) * (other_growth as u128) * (capacity as u128);
    left > right
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> MarkdownStyles {
        let mut styles = MarkdownStyles::new();
        for (index, role) in MarkdownRole::ALL.iter().enumerate() {
            styles.set_fg(*role, [index as u8 + 1, 10, 20, 255]);
        }
        styles
    }

    fn flow(content: &str) -> MarkdownFlow {
        let mut flow = MarkdownFlow::new(MarkdownOptions::default());
        flow.set_styles(palette());
        flow.set_content(content);
        flow
    }

    fn rendered(flow: &MarkdownFlow, width: usize) -> Vec<String> {
        flow.render(width).into_iter().map(|row| row.text()).collect()
    }

    fn inline_text(content: &str) -> String {
        let flow = flow(content);
        let BlockBody::Inline(lines) = &flow.blocks()[0].body else { panic!("inline body") };
        lines[0].text.clone()
    }

    /// MD-001. Upstream: `Markdown.ts:592-715` (inline), `856-1005` (quote and
    /// list), `1159-1186` (rule and fence), `1253-1267` (margins), `1341-1374`
    /// (top-level blocks), `1430-1561` + `TextTable.ts:663-750` (grid tables);
    /// `packages/theme/src/tui/syntax.ts:46-71` (roles/attributes).
    #[test]
    fn md_001_block_composition_matches_upstream_rules() {
        let source = "# Title\n\nplain **bold** *ital* ~~gone~~ `code` [label](https://x.test)\n\n> quoted\n\n- one\n- two\n\n1. first\n2. second\n\n---\n\n```rust\nlet x = 1;\n```\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n";
        let flow = flow(source);
        let kinds: Vec<&BlockKind> = flow.blocks().iter().map(|block| &block.kind).collect();
        assert!(matches!(kinds[0], BlockKind::Heading(1)));
        assert!(matches!(kinds[1], BlockKind::Paragraph));
        assert!(matches!(kinds[2], BlockKind::BlockQuote));
        assert!(matches!(kinds[3], BlockKind::List { ordered: false, .. }));
        assert!(matches!(kinds[4], BlockKind::List { ordered: true, start: 1 }));
        assert!(matches!(kinds[5], BlockKind::Rule));
        assert!(matches!(kinds[6], BlockKind::Code { lang: Some(lang) } if lang == "rust"));
        assert!(matches!(kinds[7], BlockKind::Table));

        // Heading conceal removes the marker; h1 adds underline on top of bold
        // (`syntax.ts:58`).
        let BlockBody::Inline(heading) = &flow.blocks()[0].body else { panic!("heading body") };
        assert_eq!(heading[0].text, "Title");
        assert!(heading[0].spans.iter().all(|span| span.role == MarkdownRole::Heading1));
        assert!(flow.styles().get(MarkdownRole::Heading1).attrs.bold);
        assert!(flow.styles().get(MarkdownRole::Heading1).attrs.underline);
        assert!(flow.styles().get(MarkdownRole::Heading).attrs.bold);
        assert!(!flow.styles().get(MarkdownRole::Heading).attrs.underline);

        // Inline spans: markers concealed; the link keeps its target because
        // OC2 has no hyperlink capability (`Markdown.ts:666-680`).
        let BlockBody::Inline(paragraph) = &flow.blocks()[1].body else { panic!("paragraph body") };
        assert_eq!(paragraph[0].text, "plain bold ital gone code label (https://x.test)");
        let roles: Vec<MarkdownRole> = paragraph[0].spans.iter().map(|span| span.role).collect();
        assert!(roles.contains(&MarkdownRole::Strong));
        assert!(roles.contains(&MarkdownRole::Emphasis));
        assert!(roles.contains(&MarkdownRole::Strikethrough));
        assert!(roles.contains(&MarkdownRole::InlineCode));
        assert!(roles.contains(&MarkdownRole::LinkLabel));
        assert!(roles.contains(&MarkdownRole::LinkUrl));
        let strong = paragraph[0].spans.iter().find(|span| span.role == MarkdownRole::Strong).unwrap();
        assert_eq!(&paragraph[0].text[strong.start..strong.end], "bold");
        assert_eq!(flow.styles().get(MarkdownRole::Strong).attrs.bold, true);
        assert_eq!(flow.styles().get(MarkdownRole::Emphasis).attrs.italic, true);
        assert_eq!(MarkdownRole::Strikethrough.token(), "text.muted");

        // Ordered list markers are numbered consecutively and share a width
        // (`Markdown.ts:913-924`).
        let BlockBody::List(items) = &flow.blocks()[4].body else { panic!("list body") };
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].marker, "1.");
        assert_eq!(items[1].marker, "2.");
        assert_eq!(items[0].marker_width, 2);

        // Fence content excludes fence lines and info string.
        let BlockBody::Code { lines, lang } = &flow.blocks()[6].body else { panic!("code body") };
        assert_eq!(lang.as_deref(), Some("rust"));
        assert_eq!(lines, &vec!["let x = 1;".to_owned()]);

        // Grid table: full-width border row, single-cell horizontal padding,
        // header + inner + bottom borders.
        let rows = rendered(&flow, 40);
        let border = rows.iter().find(|row| row.starts_with('┌')).expect("grid top border");
        assert_eq!(cells(border), 40);
        let header = rows.iter().find(|row| row.contains(" a ")).expect("header row");
        assert!(header.starts_with('│') && header.ends_with('│'));
        assert!(rows.iter().any(|row| row.starts_with('├')));
        assert!(rows.iter().any(|row| row.starts_with('└')));
        let rule = rows
            .iter()
            .find(|row| !row.is_empty() && row.chars().all(|ch| ch == '─'))
            .expect("horizontal rule");
        assert_eq!(cells(rule), 40);
        assert!(rows.iter().any(|row| row == "│ quoted"));
    }

    /// MD-002. Upstream: unfinished fences stay code blocks with every text byte
    /// (`markdown-parser.ts:13-79`, `Markdown.ts:1171-1186`), split Unicode is
    /// never replaced, empty chunks are no-ops (`Markdown.ts:430-437`), and
    /// unfinished inline markers stay literal (`Markdown.ts:625-646`).
    #[test]
    fn md_002_unfinished_content_and_split_unicode_preserve_text() {
        let mut flow = MarkdownFlow::new(MarkdownOptions { streaming: true, ..MarkdownOptions::default() });
        flow.set_content("before\n```rust\nlet x = 1;\n");
        let code = flow
            .blocks()
            .iter()
            .find(|block| matches!(block.kind, BlockKind::Code { .. }))
            .expect("unfinished fence still a code block");
        let BlockBody::Code { lines, .. } = &code.body else { panic!("code body") };
        assert_eq!(lines, &vec!["let x = 1;".to_owned()]);

        // Split UTF-8 scalar across transport chunks.
        let mut split = MarkdownFlow::new(MarkdownOptions::default());
        for byte in "界".as_bytes().chunks(2) {
            split.push_bytes(byte);
        }
        assert_eq!(split.content(), "界");
        assert_eq!(rendered(&split, 10), vec!["界".to_owned()]);
        split.push_bytes(b"");
        split.push_chunk("");
        split.push_chunk("   ");
        assert_eq!(split.content(), "界   ");

        // Unfinished inline formatting stays literal.
        assert_eq!(inline_text("**bold** *unfinished"), "bold *unfinished");
        assert_eq!(inline_text("`unfinished"), "`unfinished");
        assert_eq!(inline_text("snake_case_name"), "snake_case_name");
        assert_eq!(inline_text("~~open"), "~~open");

        // Streaming keeps the trailing tokens unstable and re-lexes them, so a
        // reused-prefix count is conservative until the stream finishes
        // (`markdown-parser.ts:47-48`).
        let mut streaming = MarkdownFlow::new(MarkdownOptions { streaming: true, ..MarkdownOptions::default() });
        streaming.set_content("```sh\necho one\n");
        streaming.set_content("```sh\necho one\necho two\n```\n");
        assert_eq!(streaming.reused_tokens(), 0);
        streaming.finish();
        assert_eq!(streaming.reused_tokens(), 1);
    }

    /// MD-003. Upstream: `Markdown.ts:2170-2192` and
    /// `markdown-parser.ts:13-79` re-parse the final content; block
    /// reconciliation (`Markdown.ts:1995-2085`) keeps one block per token, so
    /// both completion orders end with exactly one complete fence block.
    #[test]
    fn md_003_both_completion_orders_finish_one_complete_fence_block() {
        let final_content = "```sh\necho one\necho two\n```\n";
        let mut content_then_finish =
            MarkdownFlow::new(MarkdownOptions { streaming: true, ..MarkdownOptions::default() });
        content_then_finish.set_content("```sh\necho one\n");
        content_then_finish.set_content(final_content);
        content_then_finish.finish();

        let mut finish_then_content =
            MarkdownFlow::new(MarkdownOptions { streaming: true, ..MarkdownOptions::default() });
        finish_then_content.set_content("```sh\necho one\n");
        finish_then_content.finish();
        finish_then_content.set_content(final_content);

        for flow in [&content_then_finish, &finish_then_content] {
            let code_blocks: Vec<&MarkdownBlock> = flow
                .blocks()
                .iter()
                .filter(|block| matches!(block.kind, BlockKind::Code { .. }))
                .collect();
            assert_eq!(code_blocks.len(), 1, "exactly one fence block");
            let BlockBody::Code { lines, .. } = &code_blocks[0].body else { panic!("code body") };
            assert_eq!(lines, &vec!["echo one".to_owned(), "echo two".to_owned()]);
            assert_eq!(flow.render(40).len(), 2, "two content rows, no stale block");
        }
        assert_eq!(content_then_finish.render(40), finish_then_content.render(40));
    }

    /// MD-004. Upstream: `Markdown.ts:466-472` (conceal re-renders styles, block
    /// state kept), `TextTable.ts:663-750` + `text-table-width.ts:15-97` (narrow
    /// reflow fits columns), `Markdown.ts:1171-1186` (code geometry is the
    /// literal content).
    #[test]
    fn md_004_source_toggle_and_narrow_reflow_keep_anchors_and_geometry() {
        let source = "# Head\n\nplain **bold** text\n\n| alpha beta | gamma |\n| --- | --- |\n| a long value here | 2 |\n\n```\nlet x = 1; let y = 2;\n```\n";
        let flow = flow(source);
        let anchors: Vec<(usize, usize)> = (0..flow.blocks().len()).map(|id| flow.anchor(id).unwrap()).collect();
        let rendered_text = rendered(&flow, 30);
        assert!(rendered_text.iter().any(|row| row.starts_with('┌')));
        assert!(rendered_text.contains(&"Head".to_owned()));
        assert!(rendered_text.contains(&"plain bold text".to_owned()));

        let mut toggled = flow.clone();
        toggled.set_conceal(false);
        let anchors_after: Vec<(usize, usize)> =
            (0..toggled.blocks().len()).map(|id| toggled.anchor(id).unwrap()).collect();
        assert_eq!(anchors, anchors_after, "selection/scroll anchors survive the toggle");
        assert!(toggled.blocks().iter().all(|block| block.preserved));
        let source_rows = rendered(&toggled, 30);
        assert!(source_rows.contains(&"# Head".to_owned()));
        assert!(source_rows.contains(&"plain **bold** text".to_owned()));

        // Narrow width: fence bytes are unchanged; the table fits the width
        // instead of overflowing.
        let narrow = rendered(&flow, 16);
        assert!(narrow.contains(&"let x = 1; let y = 2;".to_owned()));
        let border = narrow.iter().find(|row| row.starts_with('┌')).expect("table border");
        assert_eq!(cells(border), 16);
        // Every bordered table row keeps the full grid width, including rows
        // whose cells have different wrapped heights.
        assert!(narrow
            .iter()
            .filter(|row| matches!(row.chars().next(), Some('│' | '├' | '└')))
            .all(|row| cells(row) == 16));
        let wide = rendered(&flow, 60);
        assert!(wide.contains(&"let x = 1; let y = 2;".to_owned()));
        assert_eq!(cells(wide.iter().find(|row| row.starts_with('┌')).unwrap()), 60);
        // Cell text survives reflow. At 16 cells the second column only has
        // three content cells, so "gamma" hard-wraps ("gam" + "ma"); at a wide
        // width the same cells stay on one line.
        let table_text: String = narrow.iter().filter(|row| row.starts_with('│')).cloned().collect();
        assert!(table_text.contains("alpha"));
        assert!(table_text.contains("gam") && table_text.contains("ma"));
        let wide_text: String = wide.iter().filter(|row| row.starts_with('│')).cloned().collect();
        assert!(wide_text.contains("gamma"));
        assert!(wide_text.contains("a long value here"));
    }

    /// MD-005. Upstream: `routes/session/thinking-syntax.ts:3-8` replaces only
    /// the foreground of every registered style; `Markdown.ts:466-472` and
    /// `2280-2370` re-render styles without rebuilding block state.
    #[test]
    fn md_005_theme_switch_recolors_without_losing_text_or_attributes() {
        let source = "# Head\n\n**bold** *ital* `code`\n";
        let mut flow = flow(source);
        let before: Vec<String> = rendered(&flow, 40);
        let before_blocks = flow.blocks().len();

        let mut recolored = MarkdownStyles::new();
        for role in MarkdownRole::ALL {
            recolored.set_fg(role, [200, 100, 50, 255]);
        }
        flow.set_styles(recolored);
        assert_eq!(rendered(&flow, 40), before, "no text loss on theme switch");
        assert_eq!(flow.blocks().len(), before_blocks);
        assert_eq!(flow.styles().get(MarkdownRole::Strong).fg, [200, 100, 50, 255]);

        // Reasoning uses muted foreground while retaining non-color attributes.
        let thinking = palette().thinking([90, 90, 90, 255]);
        for role in MarkdownRole::ALL {
            assert_eq!(thinking.get(role).fg, [90, 90, 90, 255]);
            assert_eq!(thinking.get(role).attrs, palette().get(role).attrs);
        }
        assert!(thinking.get(MarkdownRole::Strong).attrs.bold);
        assert!(thinking.get(MarkdownRole::Emphasis).attrs.italic);
        assert!(thinking.get(MarkdownRole::Link).attrs.underline);
    }

    /// MD-006. Upstream: `packages/tui/src/context/thinking.ts:9-14`
    /// (`reasoningSummary`) and `routes/session/message-parts.tsx:119-121`
    /// (`reasoningContent`); the detailed assertions live in `medown::` tests.
    #[test]
    fn md_006_reasoning_summary_metadata_is_recognised() {
        let summary = crate::medown::reasoning_summary("**Inspecting PR workflow**\n\nWe looked at the diff.");
        assert_eq!(summary.title.as_deref(), Some("Inspecting PR workflow"));
        assert_eq!(summary.body, "We looked at the diff.");
        let prose = crate::medown::reasoning_summary("**Not** a title block\nsecond line");
        assert_eq!(prose.title, None);
        assert_eq!(crate::medown::reasoning_body("[REDACTED]"), None);
        assert_eq!(crate::medown::reasoning_content("[REDACTED]  [REDACTED]"), "");
    }

    /// MD-007. Upstream: `packages/tui/src/plugin/markdown.ts:8-17` (last-wins
    /// effective handler map, shallow equality) and `Markdown.ts:503-511`
    /// (a changed `renderNode` clears and rebuilds block state).
    #[test]
    fn md_007_plugin_toggle_preserves_block_identity_and_handler_changes_rebuild() {
        let mut flow = flow("```mermaid\ngraph TD\n```\n");
        let ids: Vec<usize> = flow.blocks().iter().map(|block| block.id).collect();
        let epoch = flow.renderer_epoch();
        assert!(flow.blocks()[0].fence_handler.is_none());

        let mut registry = MarkdownPluginRegistry::new();
        registry.add_plugin("mermaid-plugin");
        registry.add_plugin("other-plugin");
        registry.set_handler("mermaid-plugin", "mermaid", "mermaid-renderer");
        flow.set_fence_handlers(&registry);
        assert_eq!(flow.renderer_epoch(), epoch + 1);
        assert_eq!(flow.blocks()[0].fence_handler.as_deref(), Some("mermaid-renderer"));
        assert_eq!(flow.blocks().iter().map(|block| block.id).collect::<Vec<_>>(), ids);

        // Toggling an unrelated plugin changes nothing effective.
        let epoch = flow.renderer_epoch();
        registry.set_active("other-plugin", false);
        flow.set_fence_handlers(&registry);
        assert_eq!(flow.renderer_epoch(), epoch);
        assert_eq!(flow.blocks()[0].fence_handler.as_deref(), Some("mermaid-renderer"));

        // Removing the handler updates the existing block (safe fallback).
        registry.remove_handler("mermaid-plugin", "mermaid");
        flow.set_fence_handlers(&registry);
        assert_eq!(flow.blocks()[0].fence_handler, None);
        assert_eq!(flow.blocks().len(), 1);
        assert_eq!(rendered(&flow, 20), vec!["graph TD".to_owned()]);

        // A handler change is a different effective map and bumps the epoch.
        registry.set_handler("mermaid-plugin", "mermaid", "second");
        let epoch = flow.renderer_epoch();
        registry.set_handler("mermaid-plugin", "mermaid", "third");
        flow.set_fence_handlers(&registry);
        assert_eq!(flow.renderer_epoch(), epoch + 1);
        assert_eq!(flow.blocks()[0].fence_handler.as_deref(), Some("third"));
    }

    #[test]
    fn nested_lists_indent_by_parent_marker_width() {
        let flow = flow("- outer\n  - inner\n    - deeper\n- next\n");
        let rows = rendered(&flow, 40);
        assert_eq!(rows, vec!["- outer", "  - inner", "    - deeper", "- next"]);
        let BlockBody::List(items) = &flow.blocks()[0].body else { panic!("list body") };
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].children.len(), 1);
        assert_eq!(items[0].children[0].children.len(), 1);
    }

    #[test]
    fn table_cells_wrap_and_normalize_missing_cells() {
        let flow = flow("| header | second |\n| --- | --- |\n| alpha beta gamma | 2 |\n| only |\n");
        let rows = rendered(&flow, 24);
        let border = rows.iter().find(|row| row.starts_with('┌')).unwrap();
        assert_eq!(cells(border), 24);
        let body_rows: Vec<&String> = rows.iter().filter(|row| row.starts_with('│')).collect();
        assert!(body_rows.iter().any(|row| row.contains("alpha")));
        // Missing trailing cell normalizes to a single space, so every row keeps
        // exactly three vertical borders.
        assert!(body_rows.iter().all(|row| row.matches('│').count() == 3));
    }

    #[test]
    fn truncation_is_char_boundary_safe_and_flagged() {
        let big = "é".repeat(MAX_FLOW_BYTES);
        let flow = flow(&big);
        assert!(flow.truncated());
        assert!(flow.content().len() <= MAX_FLOW_BYTES);
        assert!(std::str::from_utf8(flow.content().as_bytes()).is_ok());
    }

    #[test]
    fn registry_last_wins_and_version_is_stable_for_equal_maps() {
        let mut registry = MarkdownPluginRegistry::new();
        registry.add_plugin("a");
        registry.add_plugin("b");
        registry.set_handler("a", "sh", "one");
        let version = registry.version();
        registry.set_handler("b", "sh", "two");
        assert_eq!(registry.handler_for("sh"), Some("two"));
        assert_eq!(registry.version(), version + 1);
        registry.set_handler("b", "sh", "two");
        assert_eq!(registry.version(), version + 1);
    }
}
