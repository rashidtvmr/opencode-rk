#![forbid(unsafe_code)]
//! Composed information sidebar — upstream-exact geometry, section order and text.
//!
//! Pinned reference: `packages/tui/src/routes/session/sidebar.tsx:20-80`,
//! `feature-plugins/sidebar/{context,limit,mcp,todo,footer}.tsx`,
//! `component/{usage,todo-item}.tsx`, `util/{session,usage-limits,todo}.ts`,
//! `util/session.ts:46-66` (`contextUsage`) and `ui/layout.ts:1`
//! (`SESSION_SIDEBAR_WIDTH = 42`) at OpenCode `450e7b92412c780a7b5a75226bb8d49aaca24e22`.
//!
//! Geometry is derived, never guessed:
//! * pane width 42 (`SESSION_SIDEBAR_WIDTH`);
//! * outer padding top/bottom 1, left/right 2 (`sidebar.tsx:26-29`);
//! * the left/right padding is painted by the caller (`NativeView::padded`
//!   already reserves `min(width / 2, 2)` cells), so this module receives the
//!   *content* width (`content_width(42) == 38`) and budgets inside it;
//! * the body box reserves one right column for the absolute scrollbar
//!   (`sidebar.tsx:56-67`, `paddingRight={1}`), so section rows are at most
//!   [`body_width`] wide;
//! * the title box has `paddingRight={2}` (`sidebar.tsx:32`), so the title is
//!   at most [`title_width`] wide.
//!
//! Composition is structurally ordered: the model stores one optional slot per
//! contribution and [`Sidebar::render`] always emits them in the upstream claim
//! order Context, Limit, MCP, Todo (`feature-plugins/sidebar/*` append order in
//! `packages/tui/src/feature-plugins/index.ts`). A hidden contribution leaves
//! no blank placeholder row, and visible sections are separated by exactly one
//! blank row (`sidebar.tsx:69` `gap={1}`).
//!
//! The header (top padding + title + title bottom padding) and the footer
//! (top padding + getting-started + location path, `sidebar.tsx:32-49/74-76`)
//! are pinned; only the body scrolls.
//!
//! Rule sources closed here:
//! * MCP disclosure thresholds and the persisted-collapse summary
//!   (`feature-plugins/sidebar/mcp.tsx:29-49`);
//! * Todo disclosure and its `remaining` summary (`todo.tsx:25-43`);
//! * getting-started box padding, inner gaps and its independence from the
//!   location row (`footer.tsx:26-112`);
//! * the footer path menu copy/open/workspaces actions and the selection
//!   guard (`ui/working-directory-actions.tsx:16-58`).

use crate::native_theme_runtime::Slot;
use crate::native_ui_style::{
    clamp_spans, LineSpan, Role, ATTR_BOLD, ATTR_DIM, ATTR_NONE, ATTR_STRIKETHROUGH,
};
use opencode_rk_sessions::TodoStatus;

/// `SESSION_SIDEBAR_WIDTH` (`packages/tui/src/ui/layout.ts:1`).
pub const SESSION_SIDEBAR_WIDTH: usize = 42;
/// `paddingTop={1}` / `paddingBottom={1}` on the sidebar box.
pub const PADDING_TOP: usize = 1;
pub const PADDING_BOTTOM: usize = 1;
/// `paddingLeft={2}` / `paddingRight={2}` on the sidebar box.
pub const PADDING_LEFT: usize = 2;
pub const PADDING_RIGHT: usize = 2;
/// `paddingRight={2}` on the title box (`sidebar.tsx:32`).
pub const TITLE_RIGHT_INSET: usize = 2;
/// Absolute vertical scrollbar column reserved by the body box
/// (`sidebar.tsx:56-67`, `paddingRight={1}` in `sidebar.tsx:69`).
pub const SCROLLBAR_WIDTH: usize = 1;
/// `paddingTop={1}` on the pinned footer box (`sidebar.tsx:74`).
pub const FOOTER_TOP_PADDING: usize = 1;
/// `FilePath maxWidth={38}` (`feature-plugins/sidebar/footer.tsx:106`).
pub const FOOTER_PATH_MAX_WIDTH: usize = 38;
/// `dimensions().height < 22` hides getting-started (`footer.tsx:19`).
pub const ONBOARDING_MIN_HEIGHT: usize = 22;
/// MCP/Todo stay expanded at or below this count (`mcp.tsx:30/49`, `todo.tsx:26/42`).
pub const COLLAPSE_THRESHOLD: usize = 2;
/// One blank row between adjacent visible sections (`gap={1}`).
pub const BODY_GAP: usize = 1;

/// Sidebar content width (pane width minus the horizontal outer padding).
#[must_use]
pub fn content_width(pane_width: usize) -> usize {
    pane_width.saturating_sub(PADDING_LEFT + PADDING_RIGHT)
}

/// Title text width (content width minus the title box right inset).
#[must_use]
pub fn title_width(content: usize) -> usize {
    content.saturating_sub(TITLE_RIGHT_INSET)
}

/// Body text width (content width minus the scrollbar column).
#[must_use]
pub fn body_width(content: usize) -> usize {
    content.saturating_sub(SCROLLBAR_WIDTH)
}

/// Displayed cell width of `text` (wide/emoji glyphs count their cells).
#[must_use]
pub fn cell_width(text: &str) -> usize {
    crate::native_view_layout::visit_unpadded_cells(text, usize::MAX, |_, _| {})
}

/// Span covering the whole row, measured in display cells. Empty text has no
/// span: there is nothing to paint over.
fn whole_row(text: &str, slot: Slot, attributes: u32) -> Vec<LineSpan> {
    let cells = cell_width(text);
    if cells == 0 {
        Vec::new()
    } else {
        vec![LineSpan::new(0, cells, slot, attributes)]
    }
}

/// Bounded prefix of `text` that fits `max` cells.
#[must_use]
pub fn take_cells(text: &str, max: usize) -> String {
    let mut output = String::new();
    crate::native_view_layout::visit_unpadded_cells(text, max, |glyph, _| output.push_str(glyph));
    output
}

fn take_cells_end(text: &str, max: usize) -> String {
    let mut parts: Vec<(String, usize)> = Vec::new();
    crate::native_view_layout::visit_unpadded_cells(text, usize::MAX, |glyph, width| {
        parts.push((glyph.to_owned(), width))
    });
    let mut used = 0usize;
    let mut selected: Vec<String> = Vec::new();
    for (glyph, width) in parts.into_iter().rev() {
        if used + width > max {
            break;
        }
        used += width;
        selected.push(glyph);
    }
    selected.reverse();
    selected.join("")
}

/// `Math.round(tokens / limit * 100)`; `limit == 0` never yields a percentage.
#[must_use]
pub fn percent_used(tokens: u64, limit: u64) -> u32 {
    if limit == 0 {
        return 0;
    }
    let percent = (u128::from(tokens) * 100 + u128::from(limit) / 2) / u128::from(limit);
    u32::try_from(percent).unwrap_or(u32::MAX)
}

/// `Number#toLocaleString()` for a non-negative token count (en-US).
#[must_use]
pub fn format_thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

fn grouped_decimal(integral: &str) -> String {
    let mut grouped = String::with_capacity(integral.len() + integral.len() / 3);
    for (index, digit) in integral.chars().enumerate() {
        if index > 0 && (integral.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

/// `Intl.NumberFormat("en-US", { style: "currency", currency: "USD" })`
/// (`feature-plugins/sidebar/context.tsx:5-8`): always two fraction digits.
#[must_use]
pub fn format_money(value: f64) -> String {
    let value = if value.is_finite() { value } else { 0.0 };
    let sign = if value < 0.0 { "-" } else { "" };
    let text = format!("{:.2}", value.abs());
    let (integral, fraction) = text.split_once('.').unwrap_or((text.as_str(), "00"));
    format!("{sign}${}.{fraction}", grouped_decimal(integral))
}

/// `formatDollars` (`util/usage-limits.ts:78-86`): whole amounts without
/// cents, and a sliver as `<$0.01` rather than `$0.00`.
#[must_use]
pub fn format_dollars(value: f64) -> String {
    if !value.is_finite() {
        return format_money(0.0);
    }
    if value > 0.0 && value < 0.01 {
        return "<$0.01".to_owned();
    }
    let sign = if value < 0.0 { "-" } else { "" };
    let absolute = value.abs();
    let text = if absolute.fract() == 0.0 {
        format!("{absolute:.0}")
    } else {
        format!("{absolute:.2}")
    };
    let (integral, fraction) = text.split_once('.').unwrap_or((text.as_str(), ""));
    let mut money = format!("{sign}${}", grouped_decimal(integral));
    if !fraction.is_empty() {
        money.push('.');
        money.push_str(fraction);
    }
    money
}

/// Five disjoint provider token classes summed by `contextUsage`
/// (`util/session.ts:52-55`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UsageClasses {
    pub input: u64,
    pub output: u64,
    pub reasoning: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl UsageClasses {
    /// `input + output + reasoning + cache.read + cache.write`.
    #[must_use]
    pub fn total(self) -> u64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.reasoning)
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write)
    }
}

/// SIDE-04: derive the five disjoint provider classes from the wire usage.
///
/// OpenResponses `input_tokens` is the authoritative total *inclusive of
/// cached input* (`crates/server/src/auto_compaction.rs:102`), so the
/// non-cached class is `input - cached` and `cache.read` is the cached subset;
/// cache is subtracted once and re-added once, never double counted. The wire
/// reports no `cache.write`, so that class stays an honest zero rather than a
/// fabricated value. `cached` is clamped to `input` (a provider that reports
/// more cached than total cannot push `input` negative).
#[must_use]
pub fn classes_from_wire(
    input: u64,
    output: u64,
    cached: Option<u64>,
    reasoning: Option<u64>,
) -> UsageClasses {
    let cache_read = cached.unwrap_or(0).min(input);
    UsageClasses {
        input: input.saturating_sub(cache_read),
        output,
        reasoning: reasoning.unwrap_or(0),
        cache_read,
        cache_write: 0,
    }
}

/// Sidebar `Context` contribution (`context.tsx:16-41`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContextSection {
    /// Measured tokens of the last eligible assistant message; `None` when the
    /// measured total is zero (`contextUsage` returns `undefined` for
    /// `tokens <= 0`, so no token row is claimed).
    pub tokens: Option<u64>,
    /// Rounded percentage, only when the model reports a context limit.
    pub percent: Option<u32>,
    /// Session cost; the row appears only for a positive amount.
    pub cost: Option<f64>,
}

impl ContextSection {
    /// Build from an already-summed token total.
    #[must_use]
    pub fn new(tokens: Option<u64>, limit: Option<u64>, cost: Option<f64>) -> Option<Self> {
        let tokens = tokens.filter(|tokens| *tokens > 0);
        let percent = tokens.and_then(|tokens| {
            limit
                .filter(|limit| *limit > 0)
                .map(|limit| percent_used(tokens, limit))
        });
        let cost = cost.filter(|cost| cost.is_finite() && *cost > 0.0);
        if tokens.is_none() && cost.is_none() {
            return None;
        }
        Some(Self {
            tokens,
            percent,
            cost,
        })
    }

    /// Build from the five provider token classes (upstream arithmetic).
    #[must_use]
    pub fn from_classes(
        classes: UsageClasses,
        limit: Option<u64>,
        cost: Option<f64>,
    ) -> Option<Self> {
        Self::new(Some(classes.total()), limit, cost)
    }

    /// Body rows, excluding the heading.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut rows = Vec::new();
        if let Some(tokens) = self.tokens {
            rows.push(format!("{} tokens", format_thousands(tokens)));
        }
        if let Some(percent) = self.percent {
            rows.push(format!("{percent}% used"));
        }
        if let Some(cost) = self.cost {
            rows.push(format!("{} spent", format_money(cost)));
        }
        rows
    }
}

/// `usageLevel` (`util/usage-limits.ts:69-75`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageLevel {
    Plenty,
    Low,
    Out,
    Reached,
}

/// Cost of one request that fills the model's context window.
#[must_use]
pub fn request_cost(context_limit: u64, input_price_per_million: f64) -> f64 {
    (context_limit as f64 / 1_000_000.0) * input_price_per_million
}

/// Level of a model's remaining allowance.
#[must_use]
pub fn usage_level(left: f64, reached: bool, request_cost: f64) -> UsageLevel {
    if reached || left <= 0.0 {
        return UsageLevel::Reached;
    }
    if left < request_cost {
        return UsageLevel::Out;
    }
    if left < request_cost * 3.0 {
        return UsageLevel::Low;
    }
    UsageLevel::Plenty
}

/// Sidebar `Limit` contribution (`limit.tsx:12-35`). Hidden unless a limit
/// applies to the selected model; an unknown/unlimited model has no section.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LimitSection {
    pub left: f64,
    pub level: UsageLevel,
}

impl LimitSection {
    #[must_use]
    pub fn new(left: f64, level: UsageLevel) -> Self {
        Self {
            left: left.max(0.0),
            level,
        }
    }

    #[must_use]
    pub fn from_usage(left: f64, reached: bool, request_cost: f64) -> Self {
        Self::new(left, usage_level(left, reached, request_cost))
    }

    /// `level === "out" || level === "reached"` renders error text.
    #[must_use]
    pub fn dangerous(&self) -> bool {
        matches!(self.level, UsageLevel::Out | UsageLevel::Reached)
    }

    #[must_use]
    pub fn line(&self) -> String {
        format!("{} remaining", format_dollars(self.left))
    }
}

/// Upstream MCP status union as the sidebar prints it (`mcp.tsx:78-84`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpDisplayStatus {
    Connected,
    Pending,
    Failed,
    Disabled,
    NeedsAuth,
    NeedsClientRegistration,
}

impl McpDisplayStatus {
    /// Exact sidebar status label (`mcp.tsx:78-84`); the fallback branch
    /// prints the wire word, which is what `needs_client_registration` gets.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Connected => "Connected",
            Self::Pending => "Connecting",
            Self::Failed => "Error",
            Self::Disabled => "Disabled",
            Self::NeedsAuth => "Sign in",
            Self::NeedsClientRegistration => "needs_client_registration",
        }
    }

    /// Dot colour slot (`mcp.tsx:15-21`).
    #[must_use]
    pub fn dot(self) -> crate::native_theme_runtime::Slot {
        use crate::native_theme_runtime::Slot;
        match self {
            Self::Connected => Slot::Success,
            Self::Failed => Slot::Error,
            Self::NeedsAuth => Slot::Warning,
            Self::Pending | Self::Disabled | Self::NeedsClientRegistration => Slot::TextMuted,
        }
    }

    /// Row role: only a failed server uses error text (`mcp.tsx:73-76`).
    #[must_use]
    pub fn row_role(self) -> Role {
        match self {
            Self::Failed => Role::Error,
            _ => Role::Item,
        }
    }
}

/// One MCP server as the sidebar renders it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct McpItem<'a> {
    pub name: &'a str,
    pub status: McpDisplayStatus,
    /// Native extension: authenticated session inclusion (`None` = no
    /// toggle authority). Rendered as a labelled suffix, never as upstream
    /// default content (see `SIDE-15`).
    pub included: Option<bool>,
}

/// Sidebar `MCP` contribution (`mcp.tsx`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct McpSection<'a> {
    pub items: &'a [McpItem<'a>],
    pub open: bool,
}

impl<'a> McpSection<'a> {
    #[must_use]
    pub fn collapsible(&self) -> bool {
        self.items.len() > COLLAPSE_THRESHOLD
    }

    #[must_use]
    pub fn expanded(&self) -> bool {
        !self.collapsible() || self.open
    }

    /// Connected count (`mcp.tsx:10`).
    #[must_use]
    pub fn active(&self) -> usize {
        self.items
            .iter()
            .filter(|item| item.status == McpDisplayStatus::Connected)
            .count()
    }

    /// Failed + needs-auth count (`mcp.tsx:11-13`).
    #[must_use]
    pub fn errors(&self) -> usize {
        self.items
            .iter()
            .filter(|item| {
                matches!(
                    item.status,
                    McpDisplayStatus::Failed | McpDisplayStatus::NeedsAuth
                )
            })
            .count()
    }

    /// Heading text: arrow + `MCP` (+ collapsed summary, `mcp.tsx:36-47`).
    ///
    /// The disclosure arrow is gated on more than two servers
    /// (`mcp.tsx:36-38`), but the muted summary span is gated only on the
    /// persisted `open` flag (`mcp.tsx:41-46`). A list that was collapsed
    /// while long and later shrank to one/two servers therefore still shows
    /// the summary while its rows stay permanently visible
    /// (`mcp.tsx:49`, `SIDE-11`).
    #[must_use]
    pub fn heading(&self) -> String {
        let arrow = if self.collapsible() {
            if self.open {
                "▼ "
            } else {
                "▶ "
            }
        } else {
            ""
        };
        if self.open {
            format!("{arrow}MCP")
        } else {
            format!("{arrow}MCP ({})", self.summary())
        }
    }

    /// `("{n} active[, {m} error[s]]")`.
    #[must_use]
    pub fn summary(&self) -> String {
        let errors = self.errors();
        if errors > 0 {
            format!(
                "{} active, {errors} error{}",
                self.active(),
                if errors == 1 { "" } else { "s" }
            )
        } else {
            format!("{} active", self.active())
        }
    }

    /// Per-cell overrides for [`Self::heading`] (`mcp.tsx:36-47`):
    ///
    /// * the disclosure arrow is `fg={theme.text.base}` with no weight
    ///   (`mcp.tsx:37-39`);
    /// * `MCP` is wrapped in `<b>` (`mcp.tsx:41-43`), base text;
    /// * the collapsed summary — including the space before `(` — is one
    ///   `fg={theme.text.muted}` span (`mcp.tsx:43-46`).
    #[must_use]
    pub fn heading_spans(&self) -> Vec<LineSpan> {
        let text = self.heading();
        let mut spans = Vec::new();
        let mut cell = 0usize;
        if self.collapsible() {
            let arrow = if self.open { "▼ " } else { "▶ " };
            let arrow_cells = cell_width(arrow).saturating_sub(1);
            spans.push(LineSpan::new(0, arrow_cells, Slot::Text, ATTR_NONE));
            cell = cell_width(arrow);
        }
        spans.push(LineSpan::new(cell, cell_width("MCP"), Slot::Text, ATTR_BOLD));
        cell += cell_width("MCP");
        if !self.open {
            let cells = cell_width(&text).saturating_sub(cell);
            if cells > 0 {
                spans.push(LineSpan::new(cell, cells, Slot::TextMuted, ATTR_NONE));
            }
        }
        clamp_spans(&spans, cell_width(&text))
    }

    /// One server row, truncated before the fixed status (`mcp.tsx:52-88`,
    /// `SIDE-08`). `width` is the body budget.
    #[must_use]
    pub fn row(&self, item: &McpItem<'_>, width: usize) -> String {
        self.row_parts(item, width).0
    }

    /// Per-cell overrides for [`Self::row`] (`mcp.tsx:62-85`): the bullet is its
    /// own `fg={dot(status)}` text (one cell), the name is `<b>` base text, and
    /// the fixed status word is muted unless the server failed — only `failed`
    /// uses `theme.text.feedback.error.base` (`mcp.tsx:82-85`). The native
    /// `(excluded)` suffix (`SIDE-15`) stays muted.
    #[must_use]
    pub fn row_spans(&self, item: &McpItem<'_>, width: usize) -> Vec<LineSpan> {
        self.row_parts(item, width).1
    }

    fn row_parts(&self, item: &McpItem<'_>, width: usize) -> (String, Vec<LineSpan>) {
        let label = item.status.label();
        let suffix = if item.included == Some(false) {
            " (excluded)"
        } else {
            ""
        };
        // bullet + one gap + one gap before the fixed status + suffix
        let bullet = cell_width("•");
        let fixed = bullet + 1 + 1 + cell_width(label) + cell_width(suffix);
        let available = width.saturating_sub(fixed);
        let name_width = cell_width(item.name);
        // Upstream gives the name `flexGrow={1}` (sidebar/mcp.tsx:67), so the
        // fixed status word sits flush against the row's right edge and the
        // name region absorbs the slack. Reserve exactly that region so the
        // status is right-aligned instead of left-packed.
        let name_region = available;
        let name = if name_width <= name_region {
            item.name.to_owned()
        } else if name_region > 1 {
            format!("{}…", take_cells(item.name, name_region - 1))
        } else {
            String::new()
        };
        let pad = name_region.saturating_sub(cell_width(&name));
        let text = format!("• {name}{} {label}{suffix}", " ".repeat(pad));
        let mut spans = vec![LineSpan::new(0, bullet, item.status.dot(), ATTR_NONE)];
        let name_start = bullet + 1;
        if !name.is_empty() {
            let cells = cell_width(&name);
            spans.push(LineSpan::new(name_start, cells, Slot::Text, ATTR_BOLD));
        }
        let status_slot = if item.status == McpDisplayStatus::Failed {
            Slot::Error
        } else {
            Slot::TextMuted
        };
        let status_cells = cell_width(label);
        let status_start = name_start + name_region + 1;
        spans.push(LineSpan::new(status_start, status_cells, status_slot, ATTR_NONE));
        let suffix_cells = cell_width(suffix);
        if suffix_cells > 0 {
            spans.push(LineSpan::new(
                status_start + status_cells,
                suffix_cells,
                Slot::TextMuted,
                ATTR_NONE,
            ));
        }
        (text, clamp_spans(&spans, width))
    }
}

/// `todoMarker` (`util/todo.ts:24-29`).
#[must_use]
pub fn todo_marker(status: &TodoStatus) -> &'static str {
    match status {
        TodoStatus::Completed => "[✓]",
        TodoStatus::InProgress => "[•]",
        TodoStatus::Cancelled => "[-]",
        TodoStatus::Pending => "[ ]",
    }
}

/// `TodoItem` text colour (`component/todo-item.tsx:5-10`).
#[must_use]
pub fn todo_role(status: &TodoStatus) -> Role {
    match status {
        TodoStatus::InProgress => Role::Warning,
        _ => Role::Description,
    }
}

/// `TodoItem` painted foreground and attributes (`component/todo-item.tsx:5-21`).
///
/// The marker and the content share one foreground; only a cancelled todo adds
/// `TextAttributes.STRIKETHROUGH`, and only the content `<text>` carries it.
///
/// Upstream cancelled uses `theme.text.faint` (`todo-item.tsx:8`). The native
/// palette has no faint slot yet, so [`Slot::TextMuted`] stands in: upstream
/// resolves `faint` to `$text.muted` for every theme that does not define it
/// (`packages/theme/src/tui/expand.ts:44`), and the strikethrough — not the
/// colour — is what distinguishes the row.
#[must_use]
pub fn todo_span_style(status: &TodoStatus) -> (Slot, u32) {
    match status {
        TodoStatus::InProgress => (Slot::Warning, ATTR_NONE),
        TodoStatus::Cancelled => (Slot::TextMuted, ATTR_STRIKETHROUGH),
        TodoStatus::Completed | TodoStatus::Pending => (Slot::TextMuted, ATTR_NONE),
    }
}

/// One todo as the sidebar renders it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TodoRow<'a> {
    pub content: &'a str,
    pub status: TodoStatus,
}

/// Sidebar `Todo` contribution (`todo.tsx`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TodoSection<'a> {
    pub rows: &'a [TodoRow<'a>],
    pub open: bool,
}

impl<'a> TodoSection<'a> {
    #[must_use]
    pub fn collapsible(&self) -> bool {
        self.rows.len() > COLLAPSE_THRESHOLD
    }

    #[must_use]
    pub fn expanded(&self) -> bool {
        !self.collapsible() || self.open
    }

    /// `pending + in_progress` (`todo.tsx:15-17`).
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.rows
            .iter()
            .filter(|row| matches!(row.status, TodoStatus::Pending | TodoStatus::InProgress))
            .count()
    }

    /// Heading; the collapsed summary needs more than two rows
    /// (`todo.tsx:37-39`).
    #[must_use]
    pub fn heading(&self) -> String {
        let arrow = if self.collapsible() {
            if self.open {
                "▼ "
            } else {
                "▶ "
            }
        } else {
            ""
        };
        if self.collapsible() && !self.open {
            format!("{arrow}Todo ({} remaining)", self.remaining())
        } else {
            format!("{arrow}Todo")
        }
    }

    /// Per-cell overrides for [`Self::heading`] (`todo.tsx:30-40`): base-text
    /// arrow, `<b>Todo</b>`, then the muted ` ({n} remaining)` summary (its
    /// leading space included) when the more-than-two-row list is collapsed.
    #[must_use]
    pub fn heading_spans(&self) -> Vec<LineSpan> {
        let text = self.heading();
        let mut spans = Vec::new();
        let mut cell = 0usize;
        if self.collapsible() {
            let arrow = if self.open { "▼ " } else { "▶ " };
            let arrow_cells = cell_width(arrow).saturating_sub(1);
            spans.push(LineSpan::new(0, arrow_cells, Slot::Text, ATTR_NONE));
            cell = cell_width(arrow);
        }
        spans.push(LineSpan::new(cell, cell_width("Todo"), Slot::Text, ATTR_BOLD));
        cell += cell_width("Todo");
        if self.collapsible() && !self.open {
            let cells = cell_width(&text).saturating_sub(cell);
            if cells > 0 {
                spans.push(LineSpan::new(cell, cells, Slot::TextMuted, ATTR_NONE));
            }
        }
        clamp_spans(&spans, cell_width(&text))
    }

    /// Wrapped body rows: `"[x] text"`, continuation lines aligned under the
    /// text (`todo-item.tsx:15-27`, `wrapMode="word"`).
    #[must_use]
    pub fn body(&self, width: usize) -> Vec<String> {
        self.rows_with_spans(width)
            .into_iter()
            .map(|(line, _)| line)
            .collect()
    }

    /// [`Self::body`] plus the per-cell overrides of each row: the marker span
    /// (no attributes, `todo-item.tsx:14-16`) and the content span (strikethrough
    /// for a cancelled todo, `todo-item.tsx:17-24`).
    #[must_use]
    pub fn rows_with_spans(&self, width: usize) -> Vec<(String, Vec<LineSpan>)> {
        let mut rows = Vec::new();
        for row in self.rows {
            rows.extend(wrap_todo_row_spans(row, width));
        }
        rows
    }
}

/// Word wrap to a cell budget; over-long words are hard-split.
#[must_use]
pub fn wrap_cells(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut rows = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let mut word = word;
        loop {
            let separator = usize::from(!current.is_empty());
            if cell_width(&current) + separator + cell_width(word) <= width {
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(word);
                break;
            }
            if current.is_empty() {
                let head = take_cells(word, width);
                if head.is_empty() {
                    break;
                }
                let cut = head.len().min(word.len());
                rows.push(head);
                word = &word[cut..];
                if word.is_empty() {
                    break;
                }
            } else {
                rows.push(std::mem::take(&mut current));
            }
        }
    }
    if !current.is_empty() {
        rows.push(current);
    }
    rows
}

/// `truncateFilePath` (`ui/file-path.tsx:33-76`) over display cells.
#[must_use]
pub fn truncate_file_path(value: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if cell_width(value) <= max_width {
        return value.to_owned();
    }
    let drive = drive_prefix(value);
    let unc = unc_prefix(value);
    let windows = drive.is_some() || unc.is_some() || (!value.contains('/') && value.contains('\\'));
    let separator: char = match (&drive, &unc) {
        (Some((_, sep)), _) => *sep,
        (None, Some((_, _, _, sep))) => *sep,
        _ if windows => '\\',
        _ => '/',
    };
    let root = match (&drive, &unc) {
        (Some((drive, sep)), _) => format!("{drive}{sep}"),
        (None, Some((slashes, server, share, sep))) => {
            format!("{slashes}{server}{sep}{share}{sep}")
        }
        _ if value.starts_with('/') => "/".to_owned(),
        _ => String::new(),
    };
    let consumed = drive
        .as_ref()
        .map(|(drive, sep)| drive.len() + sep.len_utf8())
        .or_else(|| unc.as_ref().map(|(slashes, server, share, sep)| {
            slashes.len() + server.len() + sep.len_utf8() + share.len() + sep.len_utf8()
        }))
        .unwrap_or(root.len());
    let source = &value[consumed.min(value.len())..];
    let segments: Vec<&str> = source
        .split(|glyph| if windows { glyph == '\\' || glyph == '/' } else { glyph == separator })
        .filter(|segment| !segment.is_empty())
        .collect();
    let basename = segments.last().copied().unwrap_or(value);
    if segments.len() < 2 {
        let root_width = cell_width(&root);
        if root_width >= max_width {
            return take_cells(&root, max_width);
        }
        return format!(
            "{root}{}",
            truncate_basename(basename, max_width - root_width)
        );
    }
    let prefix = format!("{root}…{separator}");
    let prefix_width = cell_width(&prefix);
    let basename_width = max_width.saturating_sub(prefix_width);
    if basename_width == 0 {
        return take_cells(&prefix, max_width);
    }
    let compact = truncate_basename(basename, basename_width);
    if compact != basename {
        return format!("{prefix}{compact}");
    }
    let mut selected: Vec<String> = vec![basename.to_owned()];
    let separator_width = separator.len_utf8();
    let mut width = cell_width(&prefix) + cell_width(basename);
    if segments.len() >= 2 {
        for segment in segments.iter().rev().skip(1) {
            let next = cell_width(segment) + separator_width;
            if width + next > max_width {
                let available = max_width.saturating_sub(width + separator_width);
                if available > 1 {
                    selected.insert(0, format!("{}…", take_cells(segment, available - 1)));
                }
                break;
            }
            selected.insert(0, (*segment).to_owned());
            width += next;
        }
    }
    format!("{prefix}{}", selected.join(&separator.to_string()))
}

fn truncate_basename(value: &str, max_width: usize) -> String {
    if cell_width(value) <= max_width {
        return value.to_owned();
    }
    if max_width <= 1 {
        return take_cells("…", max_width);
    }
    let dot = value.rfind('.');
    let extension = match dot {
        Some(dot) if dot > 0 => &value[dot..],
        _ => "",
    };
    let extension_width = cell_width(extension);
    if extension_width >= max_width {
        return format!("…{}", take_cells_end(extension, max_width - 1));
    }
    let stem = if extension.is_empty() { value } else { &value[..dot.unwrap_or(0)] };
    format!(
        "{}…{extension}",
        take_cells(stem, max_width - extension_width - 1)
    )
}

fn drive_prefix(value: &str) -> Option<(String, char)> {
    let mut chars = value.chars();
    let letter = chars.next()?;
    if !letter.is_ascii_alphabetic() {
        return None;
    }
    if chars.next()? != ':' {
        return None;
    }
    let separator = chars.next()?;
    if separator != '\\' && separator != '/' {
        return None;
    }
    Some((format!("{letter}:"), separator))
}

type Unc<'a> = (&'a str, &'a str, &'a str, char);

fn unc_prefix(value: &str) -> Option<Unc<'_>> {
    let slashes = if value.starts_with("\\\\") {
        "\\\\"
    } else if value.starts_with("//") {
        "//"
    } else {
        return None;
    };
    let separator = if slashes == "//" { '/' } else { '\\' };
    let rest = &value[slashes.len()..];
    let mut parts = rest.split(|glyph| glyph == '\\' || glyph == '/');
    let server = parts.next().unwrap_or("");
    let share = parts.next().unwrap_or("");
    if server.is_empty() || share.is_empty() {
        return None;
    }
    Some((slashes, server, share, separator))
}

/// Pinned footer location (`footer.tsx:87-92/104-109`): the formatted
/// directory with a `:branch` suffix, truncated to 38 cells by `FilePath`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Footer<'a> {
    pub directory: &'a str,
    pub branch: Option<&'a str>,
    pub hovered: bool,
}

impl<'a> Footer<'a> {
    #[must_use]
    pub fn label(&self) -> String {
        footer_label(self.directory, self.branch, FOOTER_PATH_MAX_WIDTH)
    }

    /// Hovered path uses base text, otherwise muted (`footer.tsx:107`).
    #[must_use]
    pub fn role(&self) -> Role {
        if self.hovered {
            Role::Item
        } else {
            Role::Description
        }
    }

    /// Working-directory menu for this pinned footer
    /// (`working-directory-actions.tsx:16-58`).
    #[must_use]
    pub fn menu(&self, selection_active: bool, can_move: bool) -> Option<Vec<FooterMenuItem>> {
        footer_menu(Some(self.directory), selection_active, can_move)
    }
}

/// `branch ? `${path}:${branch}` : path`, then `truncateFilePath`.
#[must_use]
pub fn footer_label(directory: &str, branch: Option<&str>, max_width: usize) -> String {
    let value = match branch.filter(|branch| !branch.is_empty()) {
        Some(branch) => format!("{directory}:{branch}"),
        None => directory.to_owned(),
    };
    truncate_file_path(&value, max_width)
}

/// Title of the working-directory menu (`working-directory-actions.tsx:22`).
pub const FOOTER_MENU_TITLE: &str = "Working directory";

/// One entry of the footer path menu (`working-directory-actions.tsx:24-54`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FooterAction {
    /// `location.copy` — writes the directory to the clipboard and toasts
    /// `Path copied to clipboard`.
    CopyPath,
    /// `location.open` — hands the directory to the system file manager.
    OpenFolder,
    /// `session.move` — only offered when the caller can move the session.
    Workspaces,
}

impl FooterAction {
    /// Exact menu title (`working-directory-actions.tsx:25/37/48`).
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::CopyPath => "Copy path",
            Self::OpenFolder => "Open folder",
            Self::Workspaces => "Workspaces",
        }
    }

    /// Exact menu description (`working-directory-actions.tsx:28/38/50`).
    /// `CopyPath` shows the directory itself, so it is filled by the caller.
    #[must_use]
    pub fn description(self) -> &'static str {
        match self {
            Self::CopyPath => "",
            Self::OpenFolder => "in system file manager",
            Self::Workspaces => "to another working directory",
        }
    }

    /// Keymap value carried by the menu entry.
    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            Self::CopyPath => "location.copy",
            Self::OpenFolder => "location.open",
            Self::Workspaces => "session.move",
        }
    }
}

/// One rendered menu entry: the action plus its resolved description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FooterMenuItem {
    pub action: FooterAction,
    pub title: &'static str,
    pub description: String,
}

/// `openMenu` (`working-directory-actions.tsx:16-58`).
///
/// Returns `None` (no menu, no action) when a text selection is active — the
/// footer click must not open the menu while the user is selecting text — and
/// when the working directory is unknown. `Workspaces` is offered only when
/// the caller can move the session to another directory.
#[must_use]
pub fn footer_menu(
    directory: Option<&str>,
    selection_active: bool,
    can_move: bool,
) -> Option<Vec<FooterMenuItem>> {
    if selection_active {
        return None;
    }
    let directory = directory.filter(|directory| !directory.is_empty())?;
    let mut items = vec![
        FooterMenuItem {
            action: FooterAction::CopyPath,
            title: FooterAction::CopyPath.title(),
            description: directory.to_owned(),
        },
        FooterMenuItem {
            action: FooterAction::OpenFolder,
            title: FooterAction::OpenFolder.title(),
            description: FooterAction::OpenFolder.description().to_owned(),
        },
    ];
    if can_move {
        items.push(FooterMenuItem {
            action: FooterAction::Workspaces,
            title: FooterAction::Workspaces.title(),
            description: FooterAction::Workspaces.description().to_owned(),
        });
    }
    Some(items)
}

/// Getting-started fact set (`footer.tsx:18-23`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Onboarding {
    pub dismissed: bool,
    pub integrations_known: bool,
    pub connected_provider: bool,
}

impl Onboarding {
    /// Hidden below height 22, while integrations are unknown, once
    /// dismissed, or when a provider is already connected.
    #[must_use]
    pub fn visible(&self, height: usize) -> bool {
        height >= ONBOARDING_MIN_HEIGHT
            && self.integrations_known
            && !self.dismissed
            && !self.connected_provider
    }

    /// Box rows inside the footer, including the box's own vertical padding
    /// (`footer.tsx:27-33` `paddingTop={1}`/`paddingBottom={1}`) and the
    /// `gap={1}` between the four inner children (`footer.tsx:40-70`).
    ///
    /// The outer row is `⬖` + one gap cell + the inner column, and the inner
    /// column is padded by the box's 2-cell left/right padding
    /// (`footer.tsx:31-34`), so its budget is
    /// `width - 4 (padding) - 2 (glyph + gap)`.
    #[must_use]
    pub fn lines(&self, width: usize) -> Vec<String> {
        let mut rows = Vec::new();
        let inner = width.saturating_sub(4).saturating_sub(2);
        if inner == 0 {
            // The box cannot fit its own padding and glyph; paint nothing
            // rather than emitting rows wider than the sidebar.
            return rows;
        }
        rows.push(String::new());
        rows.push(format!(
            "⬖ {:<inner$}",
            justify("Getting started", "✕", inner)
        ));
        for sentence in [
            "OpenCode includes free models so you can start immediately.",
            "Connect from 75+ providers to use other models, including Claude, GPT, Gemini etc",
        ] {
            rows.push(String::new());
            for line in wrap_cells(sentence, inner) {
                rows.push(format!("  {line}"));
            }
        }
        rows.push(String::new());
        rows.push(format!(
            "  {:<inner$}",
            justify("Connect provider", "/connect", inner)
        ));
        rows.push(String::new());
        rows
    }
}

fn justify(left: &str, right: &str, width: usize) -> String {
    let left_width = cell_width(left);
    let right_width = cell_width(right);
    if left_width + right_width >= width {
        return format!("{left} {right}");
    }
    format!("{left}{}{right}", " ".repeat(width - left_width - right_width))
}

/// One rendered sidebar row plus its click authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidebarRow {
    pub text: String,
    pub role: Role,
    pub mcp_index: Option<usize>,
    pub mcp_toggle: bool,
    /// Per-cell overrides for the cells upstream paints inside a narrower
    /// `<text>`/`<span>` (MCP dot, bold name, muted status, muted collapsed
    /// summary, faint/struck-through cancelled todo). Empty means the row is
    /// painted entirely with `role`.
    pub spans: Vec<LineSpan>,
}

impl SidebarRow {
    fn new(text: impl Into<String>, role: Role) -> Self {
        Self {
            text: text.into(),
            role,
            mcp_index: None,
            mcp_toggle: false,
            spans: Vec::new(),
        }
    }

    fn styled(text: impl Into<String>, role: Role, spans: Vec<LineSpan>) -> Self {
        Self {
            spans,
            ..Self::new(text, role)
        }
    }
}

/// The pinned contribution order (`SIDE-02`): Context, Limit, MCP, Todo.
pub const SECTION_ORDER: [&str; 4] = ["Context", "Limit", "MCP", "Todo"];

/// One optional slot per upstream sidebar contribution.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sidebar<'a> {
    pub title: Option<&'a str>,
    /// `title.pending(sessionID) && config.animations === false`
    /// (`routes/session/sidebar.tsx:41-45`): the pending rename with animations
    /// off paints the title DIM, otherwise BOLD. Only meaningful with a title.
    pub title_dim: bool,
    pub context: Option<ContextSection>,
    pub limit: Option<LimitSection>,
    pub mcp: Option<McpSection<'a>>,
    pub todo: Option<TodoSection<'a>>,
    pub footer: Option<Footer<'a>>,
    pub onboarding: Option<Onboarding>,
}

impl<'a> Sidebar<'a> {
    /// Section headings in the order they are emitted, for order assertions.
    #[must_use]
    pub fn section_headings(&self) -> Vec<&'static str> {
        let mut headings = Vec::new();
        if self.context.is_some() {
            headings.push("Context");
        }
        if self.limit.is_some() {
            headings.push("Limit");
        }
        if self.mcp.is_some_and(|mcp| !mcp.items.is_empty()) {
            headings.push("MCP");
        }
        if self.todo.is_some_and(|todo| !todo.rows.is_empty()) {
            headings.push("Todo");
        }
        headings
    }

    fn header_rows(&self, width: usize) -> Vec<SidebarRow> {
        let title_width = title_width(width);
        let mut rows = vec![SidebarRow::new(String::new(), Role::Description)];
        if let Some(title) = self.title.filter(|title| !title.trim().is_empty()) {
            // `fg={theme.text.base}` with `attributes` DIM while the rename is
            // pending and animations are off, else BOLD
            // (`sidebar.tsx:33-45`).
            let attributes = if self.title_dim { ATTR_DIM } else { ATTR_BOLD };
            let text = take_cells(title, title_width);
            let spans = whole_row(&text, Slot::Text, attributes);
            rows.push(SidebarRow::styled(text, Role::SidebarHeading, spans));
            rows.push(SidebarRow::new(String::new(), Role::Description));
        }
        rows
    }

    fn section_blocks(&self, width: usize) -> Vec<Vec<SidebarRow>> {
        let body = body_width(width);
        let mut blocks: Vec<Vec<SidebarRow>> = Vec::new();
        if let Some(context) = self.context {
            // `<b>Context</b>` over muted value rows (`context.tsx:23-37`).
            let mut block = vec![SidebarRow::styled(
                "Context",
                Role::SidebarHeading,
                whole_row("Context", Slot::Text, ATTR_BOLD),
            )];
            for line in context.lines() {
                let text = take_cells(&line, body);
                let spans = whole_row(&text, Slot::TextMuted, ATTR_NONE);
                block.push(SidebarRow::styled(text, Role::Description, spans));
            }
            blocks.push(block);
        }
        if let Some(limit) = self.limit {
            let role = if limit.dangerous() {
                Role::Error
            } else {
                Role::Description
            };
            // `fg={danger ? error : muted}` (`limit.tsx:28-32`).
            let slot = if limit.dangerous() {
                Slot::Error
            } else {
                Slot::TextMuted
            };
            let text = take_cells(&limit.line(), body);
            let spans = whole_row(&text, slot, ATTR_NONE);
            blocks.push(vec![
                SidebarRow::styled(
                    "Limit",
                    Role::SidebarHeading,
                    whole_row("Limit", Slot::Text, ATTR_BOLD),
                ),
                SidebarRow::styled(text, role, spans),
            ]);
        }
        if let Some(mcp) = self.mcp.filter(|mcp| !mcp.items.is_empty()) {
            let heading = take_cells(&mcp.heading(), body);
            let spans = clamp_spans(&mcp.heading_spans(), cell_width(&heading));
            let mut block = vec![SidebarRow {
                text: heading,
                role: Role::SidebarHeading,
                mcp_index: None,
                mcp_toggle: mcp.collapsible(),
                spans,
            }];
            if mcp.expanded() {
                for (index, item) in mcp.items.iter().enumerate() {
                    let (text, spans) = mcp.row_parts(item, body);
                    block.push(SidebarRow {
                        text,
                        role: item.status.row_role(),
                        mcp_index: Some(index),
                        mcp_toggle: false,
                        spans,
                    });
                }
            }
            blocks.push(block);
        }
        if let Some(todo) = self.todo.filter(|todo| !todo.rows.is_empty()) {
            let heading = take_cells(&todo.heading(), body);
            let spans = clamp_spans(&todo.heading_spans(), cell_width(&heading));
            let mut block = vec![SidebarRow {
                text: heading,
                role: Role::SidebarHeading,
                mcp_index: None,
                mcp_toggle: false,
                spans,
            }];
            if todo.expanded() {
                for row in todo.rows {
                    for (text, spans) in wrap_todo_row_spans(row, body) {
                        block.push(SidebarRow::styled(text, todo_role(&row.status), spans));
                    }
                }
            }
            blocks.push(block);
        }
        blocks
    }

    fn body_rows(&self, width: usize) -> Vec<SidebarRow> {
        let mut rows = Vec::new();
        for block in self.section_blocks(width) {
            if !rows.is_empty() {
                rows.extend(std::iter::repeat_n(
                    SidebarRow::new(String::new(), Role::Description),
                    BODY_GAP,
                ));
            }
            rows.extend(block);
        }
        rows
    }

    /// Rows contributed by the footer box (`sidebar.tsx:74-76`,
    /// `footer.tsx:94-112`).
    ///
    /// The box has `paddingTop={1}` and `gap={1}` between the onboarding
    /// contribution and the location row. The getting-started box is a
    /// sibling of the location row, not a child of it: it stays visible when
    /// the working directory was never observed (`footer.tsx:94-112`), and
    /// only the path row is gated on `directory()` being known.
    fn footer_rows(&self, width: usize, height: usize) -> Vec<SidebarRow> {
        let onboarding = self.onboarding.filter(|item| item.visible(height));
        if onboarding.is_none() && self.footer.is_none() {
            return Vec::new();
        }
        let mut rows = vec![
            SidebarRow::new(String::new(), Role::Description);
            FOOTER_TOP_PADDING
        ];
        if let Some(onboarding) = onboarding {
            rows.extend(
                onboarding
                    .lines(width)
                    .into_iter()
                    .map(|line| SidebarRow::new(line, Role::Description)),
            );
            if self.footer.is_some() {
                // `gap={1}` between the two footer children.
                rows.push(SidebarRow::new(String::new(), Role::Description));
            }
        }
        if let Some(footer) = self.footer {
            rows.push(SidebarRow::new(
                take_cells(&footer.label(), width),
                footer.role(),
            ));
        }
        rows
    }

    /// Largest offset the body can scroll to.
    #[must_use]
    pub fn scroll_limit(&self, width: usize, height: usize) -> usize {
        let capacity = height
            .saturating_sub(self.header_rows(width).len())
            .saturating_sub(self.footer_rows(width, height).len());
        self.body_rows(width).len().saturating_sub(capacity)
    }

    /// Body viewport in composed-row coordinates:
    /// `(body_start_row, viewport_rows, total_body_rows)`.
    ///
    /// The pinned header starts the frame and the pinned footer is composed
    /// last, so the scrolled body is exactly the middle band. The absolute
    /// scrollbar (`sidebar.tsx:56-69`) spans that band.
    #[must_use]
    pub fn body_viewport(&self, width: usize, height: usize) -> (usize, usize, usize) {
        let header = self.header_rows(width).len();
        let footer = self.footer_rows(width, height).len();
        let capacity = height.saturating_sub(header + footer);
        (header.min(height), capacity, self.body_rows(width).len())
    }

    /// Compose the sidebar: pinned header, scrolled body, pinned footer.
    #[must_use]
    pub fn render(&self, width: usize, height: usize, scroll: usize) -> Vec<SidebarRow> {
        if width == 0 || height == 0 {
            return Vec::new();
        }
        let header = self.header_rows(width);
        let footer = self.footer_rows(width, height);
        let body = self.body_rows(width);
        let capacity = height.saturating_sub(header.len() + footer.len());
        let start = scroll.min(body.len().saturating_sub(capacity));
        let mut rows = header;
        rows.extend(body.into_iter().skip(start).take(capacity));
        rows.extend(footer);
        rows.truncate(height);
        rows
    }

    /// The per-span paint plan of the same frame: `(row index, span)` pairs for
    /// the rows [`Self::render`] returns, in row order and then span order.
    ///
    /// Row indices address the returned `Vec<SidebarRow>`; spans address display
    /// cells inside that row. A row that needs no override contributes nothing,
    /// so the painter can keep using the row [`Role`] for every other row.
    #[must_use]
    pub fn render_span_styles(
        &self,
        width: usize,
        height: usize,
        scroll: usize,
    ) -> Vec<(usize, LineSpan)> {
        self.render(width, height, scroll)
            .into_iter()
            .enumerate()
            .flat_map(|(index, row)| row.spans.into_iter().map(move |span| (index, span)))
            .collect()
    }
}

fn wrap_todo_row_spans(row: &TodoRow<'_>, width: usize) -> Vec<(String, Vec<LineSpan>)> {
    let marker = todo_marker(&row.status);
    let (slot, attributes) = todo_span_style(&row.status);
    let indent = cell_width(marker) + 1;
    let marker_cells = cell_width(marker);
    let wrapped = wrap_cells(row.content, width.saturating_sub(indent));
    if wrapped.is_empty() {
        // `todo-item.tsx:14-16`: the marker `<text>` keeps the status colour and
        // no attributes even when there is no content to follow it.
        return vec![(marker.to_owned(), vec![LineSpan::new(0, marker_cells, slot, ATTR_NONE)])];
    }
    wrapped
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let text = if index == 0 {
                format!("{marker} {line}")
            } else {
                format!("{}{line}", " ".repeat(indent))
            };
            let mut spans = Vec::new();
            if index == 0 {
                spans.push(LineSpan::new(0, marker_cells, slot, ATTR_NONE));
            }
            let cells = cell_width(&line);
            if cells > 0 {
                // The content `<text>` starts after the marker and its gap
                // (`todo-item.tsx:17-24`) and owns the strikethrough.
                spans.push(LineSpan::new(indent, cells, slot, attributes));
            }
            (text, clamp_spans(&spans, width))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mcp(statuses: &[McpDisplayStatus]) -> Vec<McpItem<'static>> {
        let names = ["alpha", "beta", "gamma", "delta"];
        statuses
            .iter()
            .enumerate()
            .map(|(index, status)| McpItem {
                name: names[index % names.len()],
                status: *status,
                included: None,
            })
            .collect()
    }

    fn headings(sidebar: &Sidebar<'_>) -> Vec<String> {
        sidebar
            .render(content_width(SESSION_SIDEBAR_WIDTH), 40, 0)
            .into_iter()
            .filter(|row| row.role == Role::SidebarHeading)
            .map(|row| row.text)
            .collect()
    }

    /// SIDE-02: default visible order is Context → Limit → MCP → Todo, no
    /// matter what else is present.
    #[test]
    fn sections_render_in_upstream_order() {
        let items = mcp(&[McpDisplayStatus::Connected]);
        let todos = [TodoRow {
            content: "Task",
            status: TodoStatus::Pending,
        }];
        let sidebar = Sidebar {
            title: Some("Session"),
            title_dim: false,
            context: ContextSection::new(Some(1000), Some(200_000), Some(0.5)),
            limit: Some(LimitSection::new(5.0, UsageLevel::Plenty)),
            mcp: Some(McpSection {
                items: &items,
                open: true,
            }),
            todo: Some(TodoSection {
                rows: &todos,
                open: true,
            }),
            footer: Some(Footer {
                directory: "/projects/oc2",
                branch: Some("main"),
                hovered: false,
            }),
            onboarding: None,
        };
        // The session title shares the bold heading role, so it leads the
        // rendered heading list; contribution headings follow it unchanged.
        let rendered = headings(&sidebar);
        assert_eq!(rendered.first().map(String::as_str), Some("Session"));
        assert_eq!(
            &rendered[1..],
            ["Context", "Limit", "MCP", "Todo"],
            "upstream contribution order"
        );
        assert_eq!(sidebar.section_headings(), ["Context", "Limit", "MCP", "Todo"]);
        assert_eq!(SECTION_ORDER, ["Context", "Limit", "MCP", "Todo"]);
        let rows: Vec<String> = sidebar
            .render(content_width(SESSION_SIDEBAR_WIDTH), 40, 0)
            .into_iter()
            .map(|row| row.text)
            .collect();
        // Footer is pinned to the last row; title is the first content row.
        assert_eq!(rows.first().map(String::as_str), Some(""));
        assert_eq!(rows.get(1).map(String::as_str), Some("Session"));
        assert_eq!(rows.last().map(String::as_str), Some("/projects/oc2:main"));
        assert!(rows.contains(&"$0.50 spent".to_owned()));
        assert!(rows.contains(&"$5 remaining".to_owned()));
        assert!(rows.contains(&"[ ] Task".to_owned()));
    }

    /// SIDE-02: a hidden contribution removes the whole section (no
    /// artificial blank section, no duplicated gap).
    #[test]
    fn hidden_sections_leave_no_blank_placeholder() {
        let only_todo = Sidebar {
            todo: Some(TodoSection {
                rows: &[TodoRow {
                    content: "Only",
                    status: TodoStatus::Pending,
                }],
                open: true,
            }),
            ..Sidebar::default()
        };
        let rows: Vec<String> = only_todo
            .render(content_width(SESSION_SIDEBAR_WIDTH), 20, 0)
            .into_iter()
            .map(|row| row.text)
            .collect();
        assert_eq!(rows, ["", "Todo", "[ ] Only"]);
        assert!(only_todo.section_headings() == vec!["Todo"]);
        // Empty contributions are hidden, never titled.
        let empty = Sidebar {
            mcp: Some(McpSection {
                items: &[],
                open: true,
            }),
            todo: Some(TodoSection {
                rows: &[],
                open: true,
            }),
            ..Sidebar::default()
        };
        assert!(empty.section_headings().is_empty());
        assert!(empty
            .render(content_width(SESSION_SIDEBAR_WIDTH), 20, 0)
            .iter()
            .all(|row| row.text.trim().is_empty()));
    }

    /// SIDE-01: width 42, outer padding 2/2/1/1, title inset, body gap 1 and
    /// the absolute scrollbar column.
    #[test]
    fn geometry_matches_the_pinned_sidebar_rectangle() {
        assert_eq!(SESSION_SIDEBAR_WIDTH, 42);
        assert_eq!((PADDING_TOP, PADDING_BOTTOM), (1, 1));
        assert_eq!((PADDING_LEFT, PADDING_RIGHT), (2, 2));
        assert_eq!(content_width(42), 38);
        assert_eq!(title_width(content_width(42)), 36);
        assert_eq!(body_width(content_width(42)), 37);
        assert_eq!(BODY_GAP, 1);
        assert_eq!(FOOTER_TOP_PADDING, 1);
        assert_eq!(SCROLLBAR_WIDTH, 1);
        // Title is bounded by its 2-cell right inset; body keeps the scrollbar column.
        let long = "x".repeat(80);
        let sidebar = Sidebar {
            title: Some(&long),
            context: ContextSection::new(Some(1_000_000), Some(2_000_000), None),
            ..Sidebar::default()
        };
        let rows = sidebar.render(content_width(42), 20, 0);
        assert_eq!(rows[1].text.chars().count(), 36, "title inset");
        for row in &rows {
            assert!(
                cell_width(&row.text) <= body_width(content_width(42)).max(title_width(content_width(42))),
                "row overflows the content budget: {:?}",
                row.text
            );
        }
        // The body gap is exactly one row between two visible sections.
        let with_two = Sidebar {
            context: ContextSection::new(Some(10), None, None),
            limit: Some(LimitSection::new(1.0, UsageLevel::Plenty)),
            ..Sidebar::default()
        };
        let rows: Vec<String> = with_two
            .render(content_width(42), 20, 0)
            .into_iter()
            .map(|row| row.text)
            .collect();
        assert_eq!(rows, ["", "Context", "10 tokens", "", "Limit", "$1 remaining"]);
    }

    /// SIDE-03/CP-17: context usage arithmetic, unknown limit, zero tokens.
    #[test]
    fn context_usage_counts_five_classes_and_respects_unknown_limit() {
        let classes = UsageClasses {
            input: 120,
            output: 30,
            reasoning: 10,
            cache_read: 40,
            cache_write: 5,
        };
        assert_eq!(classes.total(), 205);
        let section = ContextSection::from_classes(classes, Some(1_000), Some(0.0))
            .expect("measured tokens are visible");
        assert_eq!(section.lines(), ["205 tokens", "21% used"]);
        // Unknown model limit: the measured total is shown without a percentage.
        let unknown = ContextSection::from_classes(classes, None, None).unwrap();
        assert_eq!(unknown.lines(), ["205 tokens"]);
        assert_eq!(unknown.percent, None);
        // Zero tokens (no usage) hides the section entirely.
        assert!(ContextSection::new(Some(0), Some(200_000), None).is_none());
        assert!(ContextSection::from_classes(UsageClasses::default(), Some(1), Some(0.0)).is_none());
        // Zero tokens with a positive cost still shows the cost row only.
        let cost_only = ContextSection::new(Some(0), Some(200_000), Some(1.5)).unwrap();
        assert_eq!(cost_only.lines(), ["$1.50 spent"]);
        // Percent rounding boundaries.
        assert_eq!(percent_used(999, 100_000), 1);
        assert_eq!(percent_used(1, 200_000), 0);
        assert_eq!(percent_used(200_000, 200_000), 100);
        assert_eq!(percent_used(400_000, 200_000), 200);
        assert_eq!(percent_used(150, 0), 0);
    }

    /// SIDE-04: the wire is inclusive-of-cached, so the class producer must
    /// subtract cache exactly once and clamp a provider that over-reports it.
    #[test]
    fn wire_usage_derives_disjoint_classes_without_double_counting_cache() {
        // input_tokens=120 already includes cached_tokens=40.
        let classes = classes_from_wire(120, 30, Some(40), Some(10));
        assert_eq!(classes.input, 80);
        assert_eq!(classes.cache_read, 40);
        assert_eq!(classes.cache_write, 0, "wire reports no cache write");
        assert_eq!(classes.reasoning, 10);
        assert_eq!(classes.total(), 80 + 30 + 10 + 40 + 0);
        // No cache reported: the whole input is non-cached.
        assert_eq!(classes_from_wire(120, 30, None, None).total(), 150);
        // Over-reported cache clamps to input, never underflows input.
        let clamped = classes_from_wire(5, 1, Some(999), None);
        assert_eq!(clamped.input, 0);
        assert_eq!(clamped.cache_read, 5);
        assert_eq!(clamped.total(), 6);
    }

    /// CP-18: number and currency formatting boundaries.
    #[test]
    fn number_and_currency_formatting_matches_upstream() {
        assert_eq!(format_thousands(999), "999");
        assert_eq!(format_thousands(1000), "1,000");
        assert_eq!(format_thousands(999_949), "999,949");
        assert_eq!(format_thousands(999_950), "999,950");
        assert_eq!(format_thousands(1_234_567), "1,234,567");
        // Intl USD default (context.tsx money formatter).
        assert_eq!(format_money(0.0), "$0.00");
        assert_eq!(format_money(0.05), "$0.05");
        assert_eq!(format_money(1.5), "$1.50");
        assert_eq!(format_money(1234.5), "$1,234.50");
        // formatDollars (usage-limits.ts).
        assert_eq!(format_dollars(0.0), "$0");
        assert_eq!(format_dollars(0.004), "<$0.01");
        assert_eq!(format_dollars(5.0), "$5");
        assert_eq!(format_dollars(5.25), "$5.25");
        assert_eq!(format_dollars(1234.0), "$1,234");
        assert_eq!(format_dollars(0.01), "$0.01");
    }

    /// SIDE-05: limit visibility and danger colour.
    #[test]
    fn limit_is_hidden_without_a_matching_limit_and_marks_out_states() {
        assert_eq!(
            usage_level(100.0, false, 0.5),
            UsageLevel::Plenty,
            "plenty left"
        );
        assert_eq!(usage_level(1.0, false, 0.5), UsageLevel::Low);
        assert_eq!(usage_level(0.25, false, 0.5), UsageLevel::Out);
        assert_eq!(usage_level(10.0, true, 0.5), UsageLevel::Reached);
        assert_eq!(usage_level(0.0, false, 0.5), UsageLevel::Reached);
        assert!(!LimitSection::new(10.0, UsageLevel::Plenty).dangerous());
        assert!(LimitSection::new(0.0, UsageLevel::Reached).dangerous());
        assert!(LimitSection::from_usage(0.0, false, 0.1).dangerous());
        assert_eq!(format_dollars(2.5), "$2.50");
        // No limit contribution → no "Limit" heading at all.
        let sidebar = Sidebar {
            context: ContextSection::new(Some(10), None, None),
            ..Sidebar::default()
        };
        assert!(!headings(&sidebar).contains(&"Limit".to_owned()));
    }

    /// SIDE-05: the painted limit rows carry the upstream colour contract
    /// (`limit.tsx:27-32`): the heading is bold base text and the remaining
    /// amount paints error red exactly for `out`/`reached`, muted otherwise.
    #[test]
    fn limit_rows_paint_error_only_for_out_and_reached() {
        for (level, dangerous) in [
            (UsageLevel::Plenty, false),
            (UsageLevel::Low, false),
            (UsageLevel::Out, true),
            (UsageLevel::Reached, true),
        ] {
            let section = LimitSection::new(3.5, level);
            let sidebar = Sidebar {
                limit: Some(section),
                ..Sidebar::default()
            };
            let rows = sidebar.render(content_width(SESSION_SIDEBAR_WIDTH), 40, 0);
            let remaining = rows
                .iter()
                .position(|row| row.text.trim() == section.line())
                .expect("limit amount row painted");
            let heading = &rows[remaining - 1];
            assert_eq!(
                heading.text.trim(),
                "Limit",
                "heading directly above the amount"
            );
            let heading_span = heading.spans.first().expect("bold heading span");
            assert_eq!(heading_span.slot, Slot::Text);
            assert_eq!(heading_span.attributes, ATTR_BOLD);
            assert_eq!(
                rows[remaining].role,
                if dangerous {
                    Role::Error
                } else {
                    Role::Description
                },
                "{level:?} row role"
            );
            let span = rows[remaining].spans.first().expect("whole-row amount span");
            assert_eq!(
                span.slot,
                if dangerous {
                    Slot::Error
                } else {
                    Slot::TextMuted
                },
                "{level:?} slot"
            );
            assert_eq!(span.attributes, ATTR_NONE);
        }
    }

    /// SIDE-06/SIDE-11: 0/1/2/3 servers and collapsed counts.
    #[test]
    fn mcp_counts_and_disclosure_match_upstream() {
        let zero: Vec<McpItem<'_>> = Vec::new();
        assert!(Sidebar {
            mcp: Some(McpSection {
                items: &zero,
                open: false
            }),
            ..Sidebar::default()
        }
        .section_headings()
        .is_empty());

        for count in 1..=2usize {
            let statuses = vec![McpDisplayStatus::Connected; count];
            let items = mcp(&statuses);
            let section = McpSection {
                items: &items,
                open: false,
            };
            assert!(!section.collapsible());
            assert!(section.expanded(), "<=2 servers stay visible");
            // `mcp.tsx:36-38` gates the arrow on more than two servers, but
            // `mcp.tsx:41-46` gates the summary only on the persisted flag.
            assert_eq!(section.heading(), format!("MCP ({count} active)"));
            assert_eq!(
                McpSection {
                    items: &items,
                    open: true
                }
                .heading(),
                "MCP",
                "no disclosure arrow at <=2 servers"
            );
            assert_eq!(section.row(&items[0], body_width(content_width(42))), "• alpha                     Connected");
        }

        let items = mcp(&[
            McpDisplayStatus::Connected,
            McpDisplayStatus::Failed,
            McpDisplayStatus::NeedsAuth,
        ]);
        let open = McpSection {
            items: &items,
            open: true,
        };
        assert!(open.collapsible());
        assert_eq!(open.heading(), "▼ MCP");
        assert_eq!(open.active(), 1);
        assert_eq!(open.errors(), 2);
        assert_eq!(
            open.row(&items[1], body_width(content_width(42))),
            "• beta                          Error"
        );
        assert_eq!(
            open.row(&items[2], body_width(content_width(42))),
            "• gamma                       Sign in"
        );
        let collapsed = McpSection {
            items: &items,
            open: false,
        };
        assert!(!collapsed.expanded());
        assert_eq!(
            collapsed.heading(),
            "▶ MCP (1 active, 2 errors)",
            "collapsed summary"
        );
        let single_error = mcp(&[
            McpDisplayStatus::Disabled,
            McpDisplayStatus::Failed,
            McpDisplayStatus::Disabled,
        ]);
        assert_eq!(
            McpSection {
                items: &single_error,
                open: false
            }
            .heading(),
            "▶ MCP (0 active, 1 error)"
        );
        assert_eq!(
            McpDisplayStatus::NeedsClientRegistration.label(),
            "needs_client_registration"
        );
        assert_eq!(McpDisplayStatus::Pending.label(), "Connecting");
        assert_eq!(
            McpDisplayStatus::Connected.dot(),
            crate::native_theme_runtime::Slot::Success
        );
        assert_eq!(
            McpDisplayStatus::Failed.dot(),
            crate::native_theme_runtime::Slot::Error
        );
        assert_eq!(
            McpDisplayStatus::NeedsAuth.dot(),
            crate::native_theme_runtime::Slot::Warning
        );
    }

    /// SIDE-06/SIDE-07/SIDE-11: a persisted collapsed state is retained in the
    /// heading, but a list that has shrunk to two entries is forced visible;
    /// all upstream status labels still use their exact row vocabulary.
    #[test]
    fn collapsed_mcp_state_keeps_small_lists_visible_and_status_words_exact() {
        let items = [
            McpItem { name: "pending", status: McpDisplayStatus::Pending, included: None },
            McpItem { name: "auth", status: McpDisplayStatus::NeedsAuth, included: None },
        ];
        let section = McpSection { items: &items, open: false };
        assert!(!section.collapsible());
        assert!(section.expanded(), "persisted collapse cannot hide <=2 rows");
        assert_eq!(section.heading(), "MCP (0 active, 1 error)");
        assert_eq!(section.row(&items[0], 30), "• pending           Connecting");
        assert_eq!(section.row(&items[1], 30), "• auth                 Sign in");
        assert_eq!(items[0].status.dot(), Slot::TextMuted);
        assert_eq!(items[1].status.dot(), Slot::Warning);
    }

    /// SIDE-08: long names truncate before the fixed status label.
    #[test]
    fn long_mcp_name_truncates_before_the_fixed_status() {
        let long = McpItem {
            name: "very-long-server-name-that-does-not-fit-in-the-sidebar",
            status: McpDisplayStatus::Connected,
            included: Some(true),
        };
        let row = McpSection {
            items: std::slice::from_ref(&long),
            open: true,
        }
        .row(&long, body_width(content_width(42)));
        assert!(cell_width(&row) <= body_width(content_width(42)), "{row}");
        assert!(row.ends_with("Connected"), "{row}");
        assert!(row.contains('…'), "{row}");
        let excluded = McpItem {
            name: "alpha",
            status: McpDisplayStatus::Failed,
            included: Some(false),
        };
        let row = McpSection {
            items: std::slice::from_ref(&excluded),
            open: true,
        }
        .row(&excluded, body_width(content_width(42)));
        assert_eq!(row, "• alpha              Error (excluded)");
    }

    /// SIDE-10/SIDE-11: todo markers, wrapping, remaining count.
    #[test]
    fn todo_markers_wrap_and_report_remaining() {
        let rows = [
            TodoRow {
                content: "Write the parity test",
                status: TodoStatus::Pending,
            },
            TodoRow {
                content: "Render markers",
                status: TodoStatus::InProgress,
            },
        ];
        let section = TodoSection {
            rows: &rows,
            open: false,
        };
        assert!(section.expanded(), "<=2 todos stay visible");
        assert_eq!(section.heading(), "Todo");
        assert_eq!(section.remaining(), 2);
        assert_eq!(todo_marker(&TodoStatus::Completed), "[✓]");
        assert_eq!(todo_marker(&TodoStatus::Cancelled), "[-]");
        assert_eq!(todo_role(&TodoStatus::InProgress), Role::Warning);
        assert_eq!(
            section.body(body_width(content_width(42))),
            ["[ ] Write the parity test", "[•] Render markers"]
        );
        // Long content wraps under the marker with an aligned continuation.
        let long = [TodoRow {
            content: "alpha beta gamma delta epsilon zeta eta theta iota kappa",
            status: TodoStatus::Pending,
        }];
        let wrapped = TodoSection {
            rows: &long,
            open: true,
        }
        .body(20);
        assert!(wrapped.len() > 1);
        assert!(wrapped[1].starts_with("    "), "hangs under the text");
        for line in &wrapped {
            assert!(cell_width(line) <= 20, "{line}");
        }
        // Three rows: collapsed heading reports the remaining count.
        let three = [
            TodoRow {
                content: "a",
                status: TodoStatus::Completed,
            },
            TodoRow {
                content: "b",
                status: TodoStatus::Pending,
            },
            TodoRow {
                content: "c",
                status: TodoStatus::InProgress,
            },
        ];
        let collapsed = TodoSection {
            rows: &three,
            open: false,
        };
        assert!(collapsed.collapsible());
        assert!(!collapsed.expanded());
        assert_eq!(collapsed.heading(), "▶ Todo (2 remaining)");
        assert_eq!(collapsed.remaining(), 2);
    }

    /// SIDE-12: onboarding height gate and dismissal.
    #[test]
    fn onboarding_appears_only_over_height_21_and_undismissed() {
        let ready = Onboarding {
            dismissed: false,
            integrations_known: true,
            connected_provider: false,
        };
        assert!(!ready.visible(21));
        assert!(ready.visible(22));
        assert!(!Onboarding {
            dismissed: true,
            ..ready
        }
        .visible(40));
        assert!(!Onboarding {
            connected_provider: true,
            ..ready
        }
        .visible(40));
        assert!(!Onboarding {
            integrations_known: false,
            ..ready
        }
        .visible(40));
        let lines = ready.lines(content_width(42));
        // Box paddingTop={1} (`footer.tsx:27`), then the ⬖ row.
        assert_eq!(lines.first().map(String::as_str), Some(""));
        assert!(lines[1].starts_with("⬖ Getting started"));
        assert!(lines[1].ends_with('✕'));
        assert!(lines.iter().any(|line| line.contains("75+ providers")));
        // The inner column has `gap={1}` between title, both sentences and
        // the connect row (`footer.tsx:40-70`).
        let connect = lines
            .iter()
            .rposition(|line| line.contains("Connect provider"))
            .expect("connect row");
        assert!(lines[connect].ends_with("/connect"));
        assert_eq!(lines[connect + 1], "", "gap before the box paddingBottom");
        // Box paddingBottom={1} (`footer.tsx:33`).
        assert_eq!(lines.last().map(String::as_str), Some(""));
        assert_eq!(
            lines.iter().filter(|line| line.is_empty()).count(),
            5,
            "padding + three inner gaps + padding: {lines:?}"
        );
        for line in &lines {
            assert!(cell_width(line) <= content_width(42).saturating_sub(4), "{line}");
        }
        // Height 21 hides the box but the pinned path remains.
        let sidebar = Sidebar {
            footer: Some(Footer {
                directory: "/w",
                branch: None,
                hovered: false,
            }),
            onboarding: Some(ready),
            ..Sidebar::default()
        };
        let short = sidebar.render(content_width(42), 21, 0);
        assert!(!short.iter().any(|row| row.text.contains("Getting started")));
        let tall = sidebar.render(content_width(42), 22, 0);
        assert!(tall.iter().any(|row| row.text.contains("Getting started")));
        assert_eq!(tall.last().map(|row| row.text.as_str()), Some("/w"));
    }

    /// SIDE-12: getting-started is a sibling of the location row, so an
    /// unknown working directory hides only the path (`footer.tsx:94-112`);
    /// the live height gate still applies to the box itself.
    #[test]
    fn onboarding_renders_without_a_known_working_directory() {
        let ready = Onboarding {
            dismissed: false,
            integrations_known: true,
            connected_provider: false,
        };
        let sidebar = Sidebar {
            onboarding: Some(ready),
            ..Sidebar::default()
        };
        let tall = sidebar.render(content_width(42), 40, 0);
        assert!(tall.iter().any(|row| row.text.contains("Getting started")));
        assert!(tall.iter().any(|row| row.text.ends_with("/connect")));
        // No location row is fabricated for the unknown directory.
        assert!(!tall.iter().any(|row| row.text.contains("Unavailable")));
        // Height 21 still suppresses the whole box.
        let short = sidebar.render(content_width(42), 21, 0);
        assert!(!short.iter().any(|row| row.text.contains("Getting started")));
    }

    /// SIDE-14: footer menu entries and the text-selection guard
    /// (`working-directory-actions.tsx:16-58`).
    #[test]
    fn footer_menu_matches_copy_open_workspaces_and_selection_guard() {
        let items = footer_menu(Some("/projects/oc2"), false, true).expect("menu");
        assert_eq!(FOOTER_MENU_TITLE, "Working directory");
        let actions: Vec<FooterAction> = items.iter().map(|item| item.action).collect();
        assert_eq!(
            actions,
            [
                FooterAction::CopyPath,
                FooterAction::OpenFolder,
                FooterAction::Workspaces
            ]
        );
        assert_eq!(items[0].title, "Copy path");
        assert_eq!(items[0].description, "/projects/oc2");
        assert_eq!(items[1].title, "Open folder");
        assert_eq!(items[1].description, "in system file manager");
        assert_eq!(items[2].title, "Workspaces");
        assert_eq!(items[2].description, "to another working directory");
        assert_eq!(FooterAction::CopyPath.value(), "location.copy");
        assert_eq!(FooterAction::OpenFolder.value(), "location.open");
        assert_eq!(FooterAction::Workspaces.value(), "session.move");
        // Without move authority the Workspaces entry does not exist.
        let fixed = footer_menu(Some("/w"), false, false).expect("menu");
        assert_eq!(fixed.len(), 2);
        assert!(fixed
            .iter()
            .all(|item| item.action != FooterAction::Workspaces));
        // A text selection prevents the menu from opening at all.
        assert!(footer_menu(Some("/w"), true, true).is_none());
        // No known directory: nothing to copy or open.
        assert!(footer_menu(None, false, true).is_none());
        assert!(footer_menu(Some(""), false, true).is_none());
        // The pinned footer delegates to the same rule.
        let footer = Footer {
            directory: "/w",
            branch: None,
            hovered: true,
        };
        assert_eq!(footer.menu(false, true), footer_menu(Some("/w"), false, true));
        assert!(footer.menu(true, true).is_none());
        // Hover only restyles the path; it never invents a menu state.
        assert_eq!(footer.role(), Role::Item);
        assert_eq!(
            Footer {
                hovered: false,
                ..footer
            }
            .role(),
            Role::Description
        );
    }

    /// SIDE-13: footer path truncation, branch suffix, platform shapes.
    #[test]
    fn footer_path_truncation_matches_file_path_rules() {
        assert_eq!(FOOTER_PATH_MAX_WIDTH, 38);
        assert_eq!(truncate_file_path("/w/short", 38), "/w/short");
        assert_eq!(
            footer_label("/projects/opencode-rk-recovery-20261005", Some("main"), 38),
            "/…/opencode-rk-recovery-20261005:main"
        );
        assert_eq!(footer_label("/w", None, 38), "/w");
        assert_eq!(footer_label("/w", Some(""), 38), "/w");
        // The suffix is part of the same max-width budget: the whole
        // `directory:branch` value is compacted as one path, so the branch
        // contributes to the width and survives as the visible tail. Upstream
        // `truncateFilePath("/Users/example/project:feature/sidebar", 20)`
        // yields `/…/project:…/sidebar` (segments split on `/`).
        let with_branch = footer_label("/Users/example/project", Some("feature/sidebar"), 20);
        assert!(cell_width(&with_branch) <= 20, "{with_branch}");
        assert_eq!(with_branch, "/…/project:…/sidebar", "{with_branch}");
        // UNIX: keep the basename, compact the middle with `…`.
        let long = "/Users/mymac/Projects/opencode-rk-recovery-20261005/crates/cli/src";
        let truncated = truncate_file_path(long, 38);
        assert!(cell_width(&truncated) <= 38, "{truncated}");
        assert!(truncated.ends_with("src"), "{truncated}");
        assert!(truncated.contains('…'), "{truncated}");
        // Basename-only overflow keeps the extension and marks the cut.
        let file = "/a/b/very-long-file-name-here.rs";
        let truncated = truncate_file_path(file, 20);
        assert!(cell_width(&truncated) <= 20, "{truncated}");
        assert!(truncated.ends_with(".rs"), "{truncated}");
        assert!(truncated.contains('…'), "{truncated}");
        // Windows drive + backslash separator.
        let drive = truncate_file_path("C:\\Users\\me\\projects\\opencode-rk", 20);
        assert!(cell_width(&drive) <= 20, "{drive}");
        assert!(drive.starts_with("C:\\…\\"), "{drive}");
        assert!(drive.ends_with("opencode-rk"), "{drive}");
        // UNC share keeps its root.
        let unc = truncate_file_path("\\\\server\\share\\a\\b\\c\\deep-folder", 24);
        assert!(cell_width(&unc) <= 24, "{unc}");
        assert!(unc.starts_with("\\\\server\\share\\"), "{unc}");
        assert!(unc.ends_with('…'), "{unc}");
        // Wide (CJK) glyphs count two cells.
        let cjk = truncate_file_path("/a/项目/很长的目录名称在这里继续延伸", 16);
        assert!(cell_width(&cjk) <= 16, "{cjk}");
        assert!(cjk.contains('…'));
        assert!(cjk.ends_with('…'), "{cjk}");
        // Emoji keep grapheme clusters intact.
        let emoji = truncate_file_path("/a/🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀", 12);
        assert!(cell_width(&emoji) <= 12, "{emoji}");
        assert!(!emoji.contains('\u{fffd}'));
        assert!(emoji.ends_with('…'), "{emoji}");
        assert_eq!(emoji.matches('🚀').count(), 4, "{emoji}");

    }

    /// SIDE-01: the header and footer are pinned while the body scrolls.
    #[test]
    fn header_and_footer_stay_pinned_while_the_body_scrolls() {
        let items = mcp(&[McpDisplayStatus::Connected; 4]);
        let sidebar = Sidebar {
            title: Some("Pinned title"),
            mcp: Some(McpSection {
                items: &items,
                open: true,
            }),
            footer: Some(Footer {
                directory: "/work",
                branch: Some("main"),
                hovered: false,
            }),
            ..Sidebar::default()
        };
        let width = content_width(42);
        let height = 8;
        let top = sidebar.render(width, height, 0);
        assert_eq!(top[0].text, "");
        assert_eq!(top[1].text, "Pinned title", "title stays at the top");
        assert_eq!(top.last().unwrap().text, "/work:main", "footer pinned");
        let limit = sidebar.scroll_limit(width, height);
        assert!(limit > 0);
        let bottom = sidebar.render(width, height, limit);
        assert_eq!(bottom.len(), height);
        assert_eq!(bottom[0].text, "");
        assert_eq!(bottom[1].text, "Pinned title");
        assert_eq!(bottom.last().unwrap().text, "/work:main");
        assert!(bottom.iter().any(|row| row.text.contains("delta")));
        assert_ne!(top, bottom, "the body really scrolled");
        // Over-scrolling clamps to the same final frame.
        assert_eq!(sidebar.render(width, height, usize::MAX), bottom);
        // Tiny heights keep the pinned header and drop body/footer rows.
        let tiny = sidebar.render(width, 2, 0);
        assert_eq!(tiny.len(), 2);
        assert_eq!(tiny[1].text, "Pinned title");
        assert!(sidebar.render(0, 10, 0).is_empty());
        assert!(sidebar.render(width, 0, 0).is_empty());
    }

    /// SIDE-15: LSP/changed-files additions are extensions, never part of the
    /// pinned default composition.
    #[test]
    fn default_composition_contains_no_extension_sections() {
        let sidebar = Sidebar {
            context: ContextSection::new(Some(5), None, None),
            ..Sidebar::default()
        };
        let rows = sidebar.render(content_width(42), 40, 0);
        for row in &rows {
            assert!(!row.text.contains("LSP"), "{:?}", row.text);
            assert!(!row.text.contains("Changed"), "{:?}", row.text);
            assert!(!row.text.contains("Files"), "{:?}", row.text);
        }
        assert_eq!(
            Sidebar::default().section_headings(),
            Vec::<&'static str>::new()
        );
    }

    // ── SIDE-07 / SIDE-10: per-span painting ──────────────────────────

    fn span_cells(row: &SidebarRow) -> usize {
        cell_width(&row.text)
    }

    /// SIDE-07: the MCP row paints its bullet with the status dot colour, the
    /// server name bold and the status word muted (error only when failed)
    /// (`mcp.tsx:62-85`).
    #[test]
    fn mcp_rows_paint_dot_name_and_status_per_span() {
        let items = mcp(&[
            McpDisplayStatus::Connected,
            McpDisplayStatus::Failed,
            McpDisplayStatus::Disabled,
            McpDisplayStatus::NeedsAuth,
        ]);
        let section = McpSection {
            items: &items,
            open: true,
        };
        let width = body_width(content_width(42));
        for item in &items {
            let text = section.row(item, width);
            let spans = section.row_spans(item, width);
            assert_eq!(spans.len(), 3, "dot, name, status: {text}");
            // Bullet: one cell, exactly the status dot colour (`mcp.tsx:64-78`).
            assert_eq!(spans[0], LineSpan::new(0, 1, item.status.dot(), ATTR_NONE));
            // Name: base text, bold, starting after the bullet and its gap.
            let name_cells = cell_width(item.name);
            assert_eq!(spans[1], LineSpan::new(2, name_cells, Slot::Text, ATTR_BOLD));
            // Status: right-aligned against the row edge (`flexGrow={1}` on the
            // name, sidebar/mcp.tsx:67); only `failed` uses the error colour.
            let bullet = 1usize;
            let label_cells = cell_width(item.status.label());
            let suffix_cells = if item.included == Some(false) {
                cell_width(" (excluded)")
            } else {
                0
            };
            let name_region = width.saturating_sub(bullet + 1 + 1 + label_cells + suffix_cells);
            let status_cell = 2 + name_region + 1;
            let status_slot = if item.status == McpDisplayStatus::Failed {
                Slot::Error
            } else {
                Slot::TextMuted
            };
            assert_eq!(
                spans[2],
                LineSpan::new(status_cell, cell_width(item.status.label()), status_slot, ATTR_NONE),
                "{text}"
            );
            assert!(spans.iter().all(|span| span.end_cell() <= cell_width(&text)));
            // The three spans name the exact cells upstream paints separately.
            assert_eq!(
                crate::native_ui_style::span_at(&spans, status_cell).map(|span| span.slot),
                Some(status_slot)
            );
        }
    }

    /// SIDE-07: the two remaining upstream states keep the muted dot and their
    /// exact labels. `pending` falls through `dot()` to `theme.text.muted`
    /// (`sidebar/mcp.tsx:20-21`) with the "Connecting" label (`:80`), and
    /// `disabled` paints its own muted dot (`:19`), its literal "Disabled"
    /// label (`:82`) and the muted status word (`:74`).
    #[test]
    fn pending_and_disabled_paint_muted_dots_with_exact_labels() {
        assert_eq!(McpDisplayStatus::Pending.dot(), Slot::TextMuted);
        assert_eq!(McpDisplayStatus::Disabled.dot(), Slot::TextMuted);
        let items = mcp(&[McpDisplayStatus::Pending, McpDisplayStatus::Disabled]);
        let section = McpSection {
            items: &items,
            open: true,
        };
        let width = body_width(content_width(42));
        for (item, label) in items.iter().zip(["Connecting", "Disabled"]) {
            let text = section.row(item, width);
            assert!(text.ends_with(label), "{text}");
            let spans = section.row_spans(item, width);
            assert_eq!(spans[0], LineSpan::new(0, 1, Slot::TextMuted, ATTR_NONE));
            assert_eq!(spans[1].slot, Slot::Text, "bold base-text name: {text}");
            assert_eq!(spans[1].attributes, ATTR_BOLD);
            assert_eq!(
                spans[2],
                LineSpan::new(
                    width - cell_width(label),
                    cell_width(label),
                    Slot::TextMuted,
                    ATTR_NONE,
                )
            );
            assert!(spans.iter().all(|span| span.end_cell() <= cell_width(&text)));
        }
    }

    /// SIDE-08/SIDE-15: the truncated name and the native `(excluded)` suffix
    /// keep their own spans, and no span reaches past the painted row.
    #[test]
    fn mcp_row_spans_survive_truncation_and_the_excluded_suffix() {
        let item = McpItem {
            name: "a-very-long-server-name-that-must-be-truncated",
            status: McpDisplayStatus::Failed,
            included: Some(false),
        };
        let section = McpSection {
            items: std::slice::from_ref(&item),
            open: true,
        };
        for width in [12usize, 20, 37] {
            let text = section.row(&item, width);
            let spans = section.row_spans(&item, width);
            assert!(spans.iter().all(|span| span.end_cell() <= cell_width(&text)));
            assert_eq!(spans[0].slot, Slot::Error, "failed dot");
            assert_eq!(spans.last().map(|span| span.slot), Some(Slot::TextMuted));
            if text.contains("a-very") {
                assert_eq!(spans[1].slot, Slot::Text, "name");
                assert_eq!(spans[1].attributes, ATTR_BOLD);
            }
            for pair in spans.windows(2) {
                assert!(pair[0].end_cell() <= pair[1].start_cell, "no overlap: {spans:?}");
            }
        }
        // No room for a name: only the bullet and the fixed status survive,
        // and neither reaches past the row budget.
        let narrow = section.row_spans(&item, 3);
        assert!(narrow.iter().all(|span| span.start_cell < 3));
    }

    /// SIDE-11: heading = base-text arrow + bold label + muted collapsed
    /// summary that includes its leading space (`mcp.tsx:36-47`,
    /// `todo.tsx:30-40`).
    #[test]
    fn collapsed_headings_paint_arrow_label_and_muted_summary() {
        let items = mcp(&[
            McpDisplayStatus::Connected,
            McpDisplayStatus::Failed,
            McpDisplayStatus::Disabled,
        ]);
        let collapsed = McpSection {
            items: &items,
            open: false,
        };
        let heading = collapsed.heading();
        assert_eq!(heading, "▶ MCP (1 active, 1 error)");
        let spans = collapsed.heading_spans();
        assert_eq!(spans[0], LineSpan::new(0, 1, Slot::Text, ATTR_NONE), "arrow");
        assert_eq!(spans[1], LineSpan::new(2, 3, Slot::Text, ATTR_BOLD), "MCP");
        assert_eq!(
            spans[2],
            LineSpan::new(5, cell_width(&heading) - 5, Slot::TextMuted, ATTR_NONE)
        );
        assert!(spans[2].covers(5), "the summary span owns the leading space");
        assert_eq!(spans[2].end_cell(), cell_width(&heading));

        // Expanded: no summary span at all, and <=2 servers drop the arrow.
        let open = McpSection {
            items: &items,
            open: true,
        };
        assert_eq!(
            open.heading_spans(),
            [LineSpan::new(0, 1, Slot::Text, ATTR_NONE), LineSpan::new(2, 3, Slot::Text, ATTR_BOLD)]
        );
        let two = mcp(&[McpDisplayStatus::Connected, McpDisplayStatus::Connected]);
        let flat = McpSection {
            items: &two,
            open: false,
        };
        assert_eq!(
            flat.heading_spans()[0],
            LineSpan::new(0, 3, Slot::Text, ATTR_BOLD),
            "no arrow at <=2 servers"
        );
        assert_eq!(
            flat.heading_spans()[1].start_cell,
            3,
            "the muted summary still owns the collapsed suffix"
        );

        let todos = [
            TodoRow {
                content: "one",
                status: TodoStatus::Completed,
            },
            TodoRow {
                content: "two",
                status: TodoStatus::Pending,
            },
            TodoRow {
                content: "three",
                status: TodoStatus::InProgress,
            },
        ];
        let todo = TodoSection {
            rows: &todos,
            open: false,
        };
        let heading = todo.heading();
        assert_eq!(heading, "▶ Todo (2 remaining)");
        let spans = todo.heading_spans();
        assert_eq!(spans[0], LineSpan::new(0, 1, Slot::Text, ATTR_NONE));
        assert_eq!(spans[1], LineSpan::new(2, 4, Slot::Text, ATTR_BOLD));
        assert_eq!(
            spans[2],
            LineSpan::new(6, cell_width(&heading) - 6, Slot::TextMuted, ATTR_NONE)
        );
    }

    /// SIDE-10: a cancelled todo is faint (`theme.text.faint`) and struck
    /// through on its content only; the marker keeps the colour without the
    /// attribute (`todo-item.tsx:5-21`).
    #[test]
    fn cancelled_todos_are_faint_and_struck_through() {
        let rows = [
            TodoRow {
                content: "Dropped",
                status: TodoStatus::Cancelled,
            },
            TodoRow {
                content: "Doing",
                status: TodoStatus::InProgress,
            },
            TodoRow {
                content: "Later",
                status: TodoStatus::Pending,
            },
            TodoRow {
                content: "Done",
                status: TodoStatus::Completed,
            },
        ];
        let section = TodoSection {
            rows: &rows,
            open: true,
        };
        let painted = section.rows_with_spans(body_width(content_width(42)));
        assert_eq!(painted.len(), 4);
        let (text, spans) = &painted[0];
        assert_eq!(text, "[-] Dropped");
        assert_eq!(spans[0], LineSpan::new(0, 3, Slot::TextMuted, ATTR_NONE));
        assert_eq!(
            spans[1],
            LineSpan::new(4, cell_width("Dropped"), Slot::TextMuted, ATTR_STRIKETHROUGH)
        );
        assert_eq!(
            crate::native_ui_style::span_at(spans, 4).map(|span| span.attributes),
            Some(ATTR_STRIKETHROUGH)
        );
        // In progress keeps the warning foreground and no strikethrough.
        assert_eq!(painted[1].1[1].slot, Slot::Warning);
        assert_eq!(painted[1].1[1].attributes, ATTR_NONE);
        // Pending/completed stay muted without attributes.
        for index in [2, 3] {
            assert_eq!(painted[index].1[1].slot, Slot::TextMuted);
            assert_eq!(painted[index].1[1].attributes, ATTR_NONE);
        }
        // `todo_role` stays the row-level fallback for the same statuses.
        assert_eq!(todo_role(&TodoStatus::Cancelled), Role::Description);
        assert_eq!(todo_span_style(&TodoStatus::Cancelled), (Slot::TextMuted, ATTR_STRIKETHROUGH));

        // A wrapped cancelled todo strikes every content row, never the indent.
        let long = [TodoRow {
            content: "alpha beta gamma delta epsilon zeta eta theta iota kappa",
            status: TodoStatus::Cancelled,
        }];
        let wrapped = TodoSection {
            rows: &long,
            open: true,
        }
        .rows_with_spans(16);
        assert!(wrapped.len() > 1);
        for (line, spans) in &wrapped {
            assert!(cell_width(line) <= 16, "{line}");
            let content = spans.last().expect("content span");
            assert_eq!(content.attributes, ATTR_STRIKETHROUGH);
            assert_eq!(content.start_cell, 4, "content hangs under the marker gap");
            assert!(content.end_cell() <= cell_width(line));
        }
    }

    /// SIDE-07/SIDE-10: `render_span_styles` addresses exactly the painted
    /// rows of `render` — every index in range, every span inside its row, and
    /// the heading/context/todo spans searchable by row text.
    #[test]
    fn render_span_styles_address_the_painted_rows() {
        let items = mcp(&[McpDisplayStatus::Connected, McpDisplayStatus::Failed]);
        let todos = [TodoRow {
            content: "Cancelled work",
            status: TodoStatus::Cancelled,
        }];
        let sidebar = Sidebar {
            title: Some("Session"),
            title_dim: false,
            context: ContextSection::new(Some(1_000), Some(200_000), Some(0.5)),
            limit: Some(LimitSection::new(0.0, UsageLevel::Reached)),
            mcp: Some(McpSection {
                items: &items,
                open: true,
            }),
            todo: Some(TodoSection {
                rows: &todos,
                open: true,
            }),
            footer: None,
            onboarding: None,
        };
        let width = content_width(SESSION_SIDEBAR_WIDTH);
        let rows = sidebar.render(width, 40, 0);
        let plan = sidebar.render_span_styles(width, 40, 0);
        assert!(!plan.is_empty());
        for (index, span) in &plan {
            let row = rows.get(*index).expect("span index addresses a painted row");
            assert!(span.cells > 0);
            assert!(span.end_cell() <= span_cells(row), "{row:?} {span:?}");
        }
        // Row text -> the spans on it.
        let spans_for = |needle: &str| -> Vec<LineSpan> {
            let index = rows
                .iter()
                .position(|row| row.text == needle)
                .unwrap_or_else(|| panic!("missing row {needle:?}"));
            plan.iter()
                .filter(|(row, _)| *row == index)
                .map(|(_, span)| *span)
                .collect()
        };
        assert_eq!(
            spans_for("Session"),
            [LineSpan::new(0, cell_width("Session"), Slot::Text, ATTR_BOLD)]
        );
        assert_eq!(
            spans_for("Context"),
            [LineSpan::new(0, 7, Slot::Text, ATTR_BOLD)]
        );
        assert_eq!(
            spans_for("1,000 tokens"),
            [LineSpan::new(0, cell_width("1,000 tokens"), Slot::TextMuted, ATTR_NONE)]
        );
        // A dangerous limit row is painted with the error slot (`limit.tsx:28-32`).
        assert_eq!(
            spans_for("$0 remaining"),
            [LineSpan::new(0, cell_width("$0 remaining"), Slot::Error, ATTR_NONE)]
        );
        let failed = spans_for("• alpha                     Connected");
        assert_eq!(failed[0].slot, Slot::Success);
        assert_eq!(failed[1].attributes, ATTR_BOLD);
        assert_eq!(failed[2].slot, Slot::TextMuted);
        assert_eq!(
            spans_for("• beta                          Error")[2].slot,
            Slot::Error,
            "a failed server paints its status with the error colour"
        );
        let cancelled = spans_for("[-] Cancelled work");
        assert_eq!(cancelled[1].attributes, ATTR_STRIKETHROUGH);
        // Blank separator rows carry no override.
        assert!(spans_for("").is_empty());
        // A frame that clips the body keeps its plan inside the painted rows.
        for height in [3usize, 6, 12, 40] {
            let frame = sidebar.render(width, height, 0);
            for (index, span) in sidebar.render_span_styles(width, height, 0) {
                let row = frame.get(index).expect("painted row");
                assert!(span.end_cell() <= span_cells(row), "height {height}: {row:?}");
            }
        }
    }
}
