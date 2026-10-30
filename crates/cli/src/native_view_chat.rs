#![forbid(unsafe_code)]
//! Native center chat view: workspace header + transcript scroll window +
//! composer prompt, rendered as bounded rows for the three-pane shell.
//!
//! Pure view state only: no IO, no Storage, no network, no signals, no
//! provider/model status. Every visible fact comes from a caller argument:
//! `workspace` (header), `transcript` (backend history rows supplied by
//! the integrator), `draft` (pure `Composer` text), `busy` (caller-
//! observed turn state). Nothing is fabricated: an idle caller bool
//! renders no turn marker at all rather than a fake "ready/connected"
//! badge.
//!
//! Reuses existing tested pieces instead of reimplementing them:
//! - [`native_transcript::strip_ansi`] sanitizes every row (CSI/SGR,
//!   OSC-8 hyperlinks, lone ESC/C1 and C0 controls except `\n`/`\t`
//!   never cross the render boundary); ordinary prose tabs are blanked here so
//!   a paste cannot shift columns, while fenced source tabs remain literal.
//! - [`native_transcript::render_lines`] word-wraps one history entry to
//!   `width` chars, char-boundary safe, hard-capped at its `MAX_WINDOW`
//!   rows (inherited ceiling: one entry contributes at most 200 rows).
//! - Display discipline mirrors `native_transcript::TranscriptPage` /
//!   `native_shell::ShellBuffer`: `scroll == 0` follows the tail; the
//!   visible window is derived from the live history on every render.
//!
//! Bounded work: render walks history from the tail and wraps only until
//! `scroll + height` rows are collected (each touched entry is sanitized
//! and cloned once, never the whole 5k-entry history), then returns only
//! the visible window — O(viewport + scroll), never O(total history) while
//! following the tail. Scroll is hard-capped at [`MAX_SCROLL_ROWS`]; one
//! entry contributes at most the `render_lines` 200-row cap.
//!
//! This file is a source proposal: it has not been compiled, formatted-
//! checked, built or executed here, and no test or E2E pass is claimed.
//! Independent review and the assembler's compiler verify it.
//!
//! Input decoding, paste framing (32 KiB gate), Ctrl-J/Shift-Enter
//! keymaps and cursor movement stay with the existing owners:
//! `native_input_decoder::TerminalInputDecoder` (raw bytes) and pure
//! `native_composer::Composer::handle_key` / `apply_paste` (edit state).
//! This module only displays the resulting `draft: &str`; it defines no
//! key protocol and performs no dispatch — the integration worker owns
//! raw key dispatch. Clearing the input after a backend turn is
//! accepted, preserving state on ambiguous failures, and never
//! auto-resending are integrator responsibilities, not view state.

use crate::native_assistant_facts::AssistantFacts;
use crate::native_transcript::{TranscriptLine, render_lines, strip_ansi};
use std::cell::Cell;

/// Max rows a single view will scroll up from the tail. Bounds both the
/// stored scroll state and the per-render wrap walk, so a hostile or
/// repeated `scroll_up` can never force an unbounded scan.
pub const MAX_SCROLL_ROWS: usize = 10_000;

/// Rendered-row resource cap: one pane render never materializes more
/// than this many rows. Extreme terminal heights and `usize::MAX`
/// callers are clamped to it before any Vec capacity, composer/header/
/// transcript work or viewport padding runs; `height` 0/1/2 keep their
/// documented priorities (nothing / composer / header + composer).
/// Mirrors the existing view row caps (e.g. the facade's
/// `MAX_BIND_FRESH_ROWS = 256`); general-review resource bound.
pub const MAX_VIEW_ROWS: usize = 256;

/// Untrusted draft bytes read for one render: a bounded UTF-8 prefix
/// (char-boundary adjust ≤ 3 bytes) is displayed only — the caller's
/// draft and the owned composer store (existing `MAX_NATIVE_DRAFT_BYTES`
/// = 32 KiB) are never mutated or truncated by rendering. Splitting
/// happens only AFTER this bound, so a newline-heavy hostile draft can
/// never allocate an unbounded row vector.
pub const MAX_DRAFT_BYTES: usize = 32 * 1024;

/// Per input row bytes read for one render (workspace header row, one
/// transcript row): a bounded UTF-8 prefix is taken BEFORE the existing
/// `strip_ansi` / `render_lines` work, so a hostile mega-row — including
/// at `width == 0`, where rows are never wrapped or cut by width —
/// cannot make sanitization allocate its input length. Honesty bound:
/// rows within the cap render exactly; content beyond it is explicitly
/// clipped for display, never silently forged or erased from state.
pub const MAX_INPUT_ROW_BYTES: usize = 32 * 1024;

/// Rendered height of one plain transcript entry at the supplied pane width.
/// Kept public for pagination anchor preservation; callers must use the same
/// bounded input and wrapping rules as the chat renderer.
pub fn transcript_entry_rows(width: usize, text: &str) -> usize {
    let clean = strip_ansi(clip_utf8_prefix_bytes(text, MAX_INPUT_ROW_BYTES)).replace('\t', " ");
    render_lines(&[TranscriptLine { id: 0, text: clean }], width).len().max(1)
}

/// Bounded UTF-8 prefix: first `max` bytes, backed off to a char
/// boundary (≤ 3 bytes) so no scalar is ever split. Render display only;
/// returns a borrowed slice — the source is never mutated or copied
/// beyond this bound.
fn clip_utf8_prefix_bytes(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Bounded center chat view: rows scrolled up from the tail of the
/// transcript (`0` = following the tail). Pure state; the caller owns the
/// history vector and passes it to [`ChatView::render`] every frame.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChatView {
    /// Rows above the tail; clamped to [`MAX_SCROLL_ROWS`].
    scroll: Cell<usize>,
    prompt_cursor: Option<usize>,
}

impl ChatView {
    pub(crate) fn set_prompt_cursor(&mut self, cursor: usize) { self.prompt_cursor = Some(cursor); }
    pub(crate) fn prompt_cursor(&self, draft: &str) -> usize {
        self.prompt_cursor.filter(|cursor| *cursor <= draft.len() && draft.is_char_boundary(*cursor))
            .unwrap_or(draft.len())
    }
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Scroll up (away from the tail) by `lines`, capped at
    /// [`MAX_SCROLL_ROWS`]. The render path synchronizes this bounded value
    /// to the current history viewport, so an overscroll cannot leave the
    /// view stuck at a stale offset after a subsequent wheel-down.
    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll.set(self.scroll.get().saturating_add(lines).min(MAX_SCROLL_ROWS));
    }

    /// Scroll down (toward the tail) by `lines`. Reaching `0` re-pins the
    /// view to the newest history.
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll.set(self.scroll.get().saturating_sub(lines));
    }

    /// Whether the bounded scroll offset is at the local ceiling.  The
    /// integrator uses this as the fetch trigger; it never changes the view
    /// itself and therefore keeps pagination separate from rendering.
    #[must_use]
    pub fn at_scroll_ceiling(&self) -> bool {
        self.scroll.get() >= MAX_SCROLL_ROWS
    }

    #[must_use]
    pub fn at_tail(&self) -> bool { self.scroll.get() == 0 }

    #[must_use]
    pub fn scroll_offset(&self) -> usize { self.scroll.get() }


    /// Re-pin to the tail: the next [`ChatView::render`] shows the newest
    /// transcript rows.
    pub fn follow_tail(&mut self) {
        self.scroll.set(0);
    }

    /// Set the tail-relative offset after a history projection has changed.
    /// Callers must derive this from the same rendered projection; this is not
    /// a second scrolling policy.
    pub(crate) fn set_scroll(&mut self, rows: usize) {
        self.scroll.set(rows.min(MAX_SCROLL_ROWS));
    }

    /// Render one center pane as exactly `height` rows (0 renders nothing)
    /// of at most `width` chars (`width == 0` disables wrapping, mirroring
    /// `native_transcript::render_lines`). Layout: workspace header row,
    /// the FULL allocated transcript viewport, composer on the pane's last
    /// rows with `"> "` as the first prompt row. The viewport is always
    /// materialized: when the scroll window holds fewer rows than the
    /// allocated `transcript_h` (empty history, short history, overscroll),
    /// the remainder is filled with bounded EMPTY rows above the window so
    /// content stays bottom-anchored directly above the composer and the
    /// prompt never shifts up under the header. Only blank rows are added
    /// — never invented transcript content. Resource bounds: `height` is
    /// clamped to [`MAX_VIEW_ROWS`] before any allocation work, and the
    /// draft / workspace inputs are read as bounded UTF-8 prefixes
    /// ([`MAX_DRAFT_BYTES`] / [`MAX_INPUT_ROW_BYTES`]) before any
    /// split or sanitize step — display only, caller state untouched.
    /// Line WORK is bounded too: only `composer budget + 1` line slices
    /// are ever collected (exact count from an allocation-free pass over
    /// the capped prefix). All rows are ANSI/control-sanitized and
    /// UTF-8 char-boundary safe.
    #[must_use]
    pub fn render(
        &self,
        width: usize,
        height: usize,
        workspace: &str,
        transcript: &[String],
        draft: &str,
        busy: bool,
    ) -> Vec<String> {
        // Resource bound first: clamp the effective height so every Vec
        // capacity, composer/header/transcript budget, padding loop and
        // final truncate below operates on at most MAX_VIEW_ROWS rows.
        let height = height.min(MAX_VIEW_ROWS);
        if height == 0 {
            return Vec::new();
        }
        // Untrusted public draft bounded BEFORE split/collect and before
        // the per-row sanitize: a bounded UTF-8 prefix is rendered for
        // display only — the caller's draft (and the owned composer
        // store) are never mutated.
        let draft = clip_utf8_prefix_bytes(draft, MAX_DRAFT_BYTES);
        // Composer budget: the EXACT line count of the ALREADY
        // byte-bounded prefix — one allocation-free O(32 KiB) iterator
        // pass (bounded by the prefix cap, not by `height`) — never a
        // full line materialization; the prompt row always fits.
        let draft_lines = draft.split('\n').count();
        let composer_h = draft_lines.clamp(1, (height / 2).max(1));
        let header_h = usize::from(height > composer_h);
        let transcript_h = height - composer_h - header_h;

        let mut rows: Vec<String> = Vec::with_capacity(height);
        // Bottom-anchored viewport: fill the allocated transcript region
        // with bounded EMPTY rows when the scroll window is short (empty
        // history, short history or overscroll), so the composer always
        // lands on the pane's last rows instead of shifting up under the
        // header. Only blank rows are added here — never invented
        // transcript content — and the window's content and scroll offset
        // selection below are untouched.
        let window = self.transcript_window(width, transcript_h, transcript);
        if header_h == 1 {
            rows.push(render_header(width, workspace, busy, self.scroll.get()));
        }
        for _ in 0..transcript_h.saturating_sub(window.len()) {
            rows.push(String::new());
        }
        rows.extend(window);
        // Row-work bound: collect ONLY `composer_h + 1` line slices (≤
        // 129 at the row cap) — enough for the rendered composer rows
        // plus the overflow-sentinel witness — and let the EXACT total
        // line count travel alongside so `+N more` stays real.
        let composer = bounded_composer_rows(draft, composer_h + 1);
        rows.extend(render_composer(
            width,
            composer_h,
            &composer.rows,
            composer.total_lines,
        ));
        rows.truncate(height);
        rows
    }

    /// Visible transcript rows: the `height` rows ending `scroll` rows
    /// above the tail. Walks history backwards from the tail, wrapping
    /// each touched entry with `native_transcript::render_lines`, and
    /// stops as soon as `scroll + height` rows exist — following the tail
    /// touches only a viewport's worth of entries, and the whole history
    /// is never cloned. The requested offset is then clamped to the oldest
    /// reachable viewport, so resize/append cannot leave a blank pane.
    fn transcript_window(&self, width: usize, height: usize, transcript: &[String]) -> Vec<String> {
        if height == 0 || transcript.is_empty() {
            return Vec::new();
        }
        let target = self.scroll.get().saturating_add(height);
        let mut rev: Vec<String> = Vec::with_capacity(target.min(4096));
        'walk: for entry in transcript.iter().rev() {
            // Per-row input bound BEFORE strip_ansi/wrap: only the O
            // (scroll + viewport) rows this window actually touches are
            // read, each as a bounded UTF-8 prefix — a hostile mega-row
            // (even at `width == 0`, never wrapped or width-cut) cannot
            // make sanitization allocate its input length, and a prefix
            // cut through an escape is finished safely by the existing
            // unterminated-sequence handling in `strip_ansi`.
            let clean =
                strip_ansi(clip_utf8_prefix_bytes(entry, MAX_INPUT_ROW_BYTES)).replace('\t', " ");
            let rows = render_lines(&[TranscriptLine { id: 0, text: clean }], width);
            for row in rows.into_iter().rev() {
                rev.push(row);
                if rev.len() >= target {
                    break 'walk;
                }
            }
        }
        rev.reverse();
        // `rev` holds the last `min(target, total)` rows of history; the
        // visible window is the first `height` of them once `scroll` tail
        // rows are skipped.
        let total = rev.len();
        if total == 0 {
            return Vec::new();
        }
        // Clamp to the oldest reachable viewport, not merely to the fixed
        // MAX_SCROLL_ROWS ceiling. This keeps short histories and resized
        // viewports visible instead of producing a blank overscrolled pane.
        let effective_scroll = self.scroll.get().min(total.saturating_sub(height));
        self.scroll.set(effective_scroll);
        let end = total - effective_scroll;
        let start = end.saturating_sub(height);
        rev[start..end].to_vec()
    }
}

    /// Workspace header row: workspace plus caller/view-fact suffixes (the
    /// busy flag the integrator observed, this view's own scroll offset),
/// composed first and then clipped as one row by the char-safe
/// [`truncate_row`] helper so the full row — suffix included — never
/// exceeds `width` in chars, including `width` 1 or 2. `width == 0`
/// disables clipping (no truncation, no underflow).
fn render_header(width: usize, workspace: &str, busy: bool, scroll: usize) -> String {
    let mut suffix = String::new();
    if scroll > 0 {
        suffix.push_str(&format!(" [+{scroll} up]"));
    }
    let workspace = strip_ansi(clip_utf8_prefix_bytes(workspace, MAX_INPUT_ROW_BYTES)).replace('\t', " ");
    let row = if busy {
        // Put the live status at the bounded pane origin so it cannot be
        // truncated by a long workspace label before becoming visible.
        format!("Thinking… {workspace}{suffix}")
    } else {
        format!("{workspace}{suffix}")
    };
    truncate_row(&row, width)
}

/// Bounded composer input: the first `take_rows` line slices of an
/// ALREADY byte-bounded draft (at most the composer budget + 1 — the +1
/// is the overflow-sentinel witness) plus the EXACT line count of that
/// bounded prefix. Production uses it in [`ChatView::render`]: the
/// collection is bounded to `take_rows`, while the exact count is a
/// separate allocation-free `.count()` pass over the ≤ 32 KiB prefix
/// (O(32 KiB), not O(height), precisely because the byte cap was applied
/// first) — so the `+N more` overflow figure is never derived from the
/// truncated vector length.
struct BoundedComposerRows<'a> {
    rows: Vec<&'a str>,
    total_lines: usize,
}

fn bounded_composer_rows(draft: &str, take_rows: usize) -> BoundedComposerRows<'_> {
    let total_lines = draft.split('\n').count();
    let rows: Vec<&str> = draft.split('\n').take(take_rows).collect();
    BoundedComposerRows { rows, total_lines }
}

/// Composer rows: `"> "` prompt on the first line (focus marker owned by
/// the integration worker's dispatch, not invented here), `  ` on
/// continuation lines, and an explicit marker replacing the last kept
/// line when the draft has more lines than the pane can show.
/// `total_lines` is the EXACT line count of the byte-bounded draft; the
/// overflow figure is computed from it, never from the bounded
/// `draft_rows` slice length (capped at `height + 1` for the sentinel
/// witness).
fn render_composer(
    width: usize,
    height: usize,
    draft_rows: &[&str],
    total_lines: usize,
) -> Vec<String> {
    let mut rows: Vec<String> = Vec::with_capacity(height);
    let shown = total_lines.min(height);
    let overflow = total_lines - shown;
    let keep = if overflow > 0 {
        shown.saturating_sub(1)
    } else {
        shown
    };
    for (i, line) in draft_rows.iter().take(keep).enumerate() {
        let prefix = if i == 0 { "> " } else { "  " };
        let text = format!("{prefix}{}", sanitize(line));
        rows.push(truncate_row(&text, width));
    }
    if overflow > 0 && keep > 0 {
        rows.push(truncate_row(
            &format!("  ... (+{overflow} more line{})", plural(overflow)),
            width,
        ));
    }
    if rows.is_empty() {
        // Degenerate single-row composer: prompt wins over the marker.
        let first = draft_rows.first().copied().unwrap_or("");
        rows.push(truncate_row(&format!("> {}", sanitize(first)), width));
    }
    rows
}

/// Sanitize one display row: ANSI/C0 controls (tab included) removed.
/// Total rows a bordered composer reserves: `2` borders + inner padding
/// above/below plus at most half the available body for draft content.
/// Shared with `NativeView::composer_region` so paint and text agree.
pub(crate) fn framed_composer_height(draft: &str, available: usize, padding: u8) -> usize {
    let overhead = 2 + 2 * usize::from(padding);
    if available <= overhead {
        return available;
    }
    let draft = clip_utf8_prefix_bytes(draft, MAX_DRAFT_BYTES);
    let lines = draft.split('\n').count();
    let content = lines.clamp(1, ((available - overhead) / 2).max(1));
    content + overhead
}

fn sanitize(text: &str) -> String {
    strip_ansi(text).replace('\t', " ")
}

/// Char-boundary safe row truncation; `width == 0` disables it.
fn truncate_row(text: &str, width: usize) -> String {
    if width == 0 {
        return text.to_owned();
    }
    let mut row = String::new();
    crate::native_view_layout::visit_unpadded_cells(text, width, |glyph, _| row.push_str(glyph));
    row
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

// Typed rendering lives above ([`ChatMessage`] / [`ChatView::render_typed`]):
// roles and parts are caller-supplied backend facts and are never inferred
// from text. The flat `transcript: &[String]` API stays for callers that only
// hold pre-formatted rows; this module still invents no history types and
// executes no tool.

/// Bound on parts displayed from one message (further parts are skipped).
pub const MAX_PARTS_PER_MESSAGE: usize = 128;
/// Bound on display rows one message may contribute.
pub const MAX_MESSAGE_ROWS: usize = MAX_VIEW_ROWS;
/// Bound in bytes for a rendered inline label (agent/model/tool name/id).
pub const MAX_LABEL_BYTES: usize = 256;

/// Role of one typed chat message, exactly as supplied by the caller's real
/// backend projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
    Tool,
    System,
}

/// One typed message part. `ToolResult::is_error` is the backend's own
/// result fact — never a guess derived from output text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatPart {
    Text(String),
    Thinking(String),
    ToolCall {
        name: String,
        id: String,
    },
    ToolResult {
        name: String,
        output: String,
        is_error: bool,
    },
    ToolState {
        name: String,
        id: String,
        status: String,
        args: String,
        output: String,
        is_error: bool,
    },
    File {
        filename: String,
        mime: String,
    },
    Subtask {
        agent: String,
        description: String,
        background: bool,
    },
    Retry {
        attempt: usize,
        message: String,
    },
    Step {
        kind: String,
        detail: String,
    },
    Compaction {
        reason: String,
    },
    Patch {
        path: String,
        diff: String,
    },
}

/// One typed chat message: caller-supplied role plus ordered parts, with the
/// optional agent/model facts the caller actually knows for this message and
/// the optional real assistant-turn execution facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub parts: Vec<ChatPart>,
    pub agent: Option<String>,
    pub model: Option<String>,
    /// Real assistant-turn execution facts (stop reason, usage tokens,
    /// interruption, error) from the native turn stream
    /// ([`crate::native_assistant_facts::AssistantFacts`]). `None` when the
    /// producer supplied no facts (e.g. replayed history whose schema carries
    /// none) — never backfilled from `MessageRecord.created_at`, wall time or
    /// the currently selected UI model.
    pub assistant_facts: Option<AssistantFacts>,
}

impl ChatMessage {
    /// Text-only convenience message.
    #[must_use]
    pub fn text(role: ChatRole, text: impl Into<String>) -> Self {
        Self {
            role,
            parts: vec![ChatPart::Text(text.into())],
            agent: None,
            model: None,
            assistant_facts: None,
        }
    }

    /// Attach real assistant-turn execution facts to this message.
    #[must_use]
    pub fn with_assistant_facts(mut self, facts: AssistantFacts) -> Self {
        self.assistant_facts = Some(facts);
        self
    }

    /// Bounded, sanitized `agent`/`model` metadata row, or `None` when the
    /// caller supplied neither fact.
    fn metadata_row(&self) -> Option<String> {
        let agent = self
            .agent
            .as_deref()
            .map(bounded_label)
            .filter(|value| !value.is_empty());
        let model = self
            .model
            .as_deref()
            .map(bounded_label)
            .filter(|value| !value.is_empty());
        match (agent, model) {
            (None, None) => None,
            (Some(agent), None) => Some(format!("  agent: {agent}")),
            (None, Some(model)) => Some(format!("  model: {model}")),
            (Some(agent), Some(model)) => Some(format!("  agent: {agent}  model: {model}")),
        }
    }
}

/// Display class of one rendered frame row. Paint metadata only: the paint
/// layer may style thinking rows muted or tool headers distinctly; this view
/// invents no colors and no backend facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatRowClass {
    /// Blank group separator before a new user/system block.
    Separator,
    /// Caller-provided agent/model fact row.
    Metadata,
    /// Assistant completion footer metadata line (the agent/model/interrupted
    /// spans of upstream `AssistantFooter`,
    /// `packages/tui/src/routes/session/index.tsx:2055-2071`).
    AssistantFooter,
    /// Assistant completion footer error notice (`Error: {message}`), muted to
    /// the error fg like upstream `theme.text.feedback.error.base`.
    FooterError,
    /// Message text (any role).
    Text,
    UserCard,
    AssistantCard,
    SystemCard,
    DiffAdded,
    DiffRemoved,
    /// Reasoning/thinking part text.
    Thinking,
    /// Tool call header (`[tool] name (id)`).
    ToolCall,
    /// Tool result header or successful output row.
    ToolResult,
    /// Errored tool result header or output row.
    ToolError,
    /// Transcript header (workspace / busy / scroll suffix).
    Header,
    /// Padding row keeping transcript content bottom-anchored.
    Blank,
    /// Composer row (`> ` prompt, continuation or overflow marker).
    Composer,
}

/// A styled row carries its explicit selectable cell interval. Decoration and
/// padding are never recovered by parsing the rendered glyphs.
pub type StyledChatRow = (String, ChatRowClass, Option<(u16, u16)>);

/// The shared ChatComposer allocation decision used by both the painter and
/// NativeView's hit-region projection. Home's legacy rail does not use this
/// helper; this is specifically the session ChatComposer path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ComposerLayout {
    pub framed: bool,
    pub rail: bool,
    pub height: usize,
}

#[must_use]
pub(crate) fn composer_layout(
    width: usize,
    height: usize,
    draft: &str,
    frame: crate::native_box_style::EffectiveBox,
) -> ComposerLayout {
    let height = height.min(MAX_VIEW_ROWS);
    let pad = usize::from(frame.padding);
    let framed = frame.has_border()
        && width > 2 + 2 * pad
        && height > 3 + 2 * pad;
    let overhead = if framed { 2 + 2 * pad } else { 0 };
    let text_width = width.saturating_sub(overhead + 2);
    let input = crate::native_prompt_layout::wrap_prompt(draft, text_width, draft.len());
    let height = input.rows.len().clamp(1, ((height.saturating_sub(overhead)) / 2).max(1)) + overhead;
    ComposerLayout { framed, rail: false, height }
}

pub(crate) fn session_composer_layout(
    width: usize,
    height: usize,
    draft: &str,
    appearance: &crate::native_box_style::BoxAppearance,
) -> ComposerLayout {
    let style = appearance.effective(crate::native_box_style::BoxId::ChatComposer);
    let height = height.min(MAX_VIEW_ROWS);
    let draft = clip_utf8_prefix_bytes(draft, MAX_DRAFT_BYTES);
    if height == 0 {
        return ComposerLayout { framed: false, rail: false, height: 0 };
    }
    if !appearance.home_composer_outline() && style.has_border()
        && style.padding == 0 && width > 5 && height >= 6
    {
        let max_input = ((height + 2) / 3).max(6).min(height - 4);
        let inset = if width < 40 { 1 } else { 2 };
        let input = crate::native_prompt_layout::wrap_prompt(draft,
            width.saturating_sub(1 + 2 * inset), draft.len()).rows.len().min(max_input).max(1);
        ComposerLayout { framed: false, rail: true, height: input + 4 }
    } else {
        composer_layout(width, height, draft, style)
    }
}

#[cfg(test)]
mod rail_prompt_bounds_tests {
    use super::*;

    #[test]
    fn rail_prompt_input_cap_respects_every_height_including_six_rows() {
        let appearance = crate::native_box_style::BoxAppearance::default();
        let multiline = "line\n".repeat(MAX_VIEW_ROWS + 10);
        for height in 0..=MAX_VIEW_ROWS + 2 {
            for width in [0, 5, 6, 39, 40, 120] {
                for draft in ["", "draft", multiline.as_str()] {
                    let layout = session_composer_layout(width, height, draft, &appearance);
                    assert!(layout.height <= height.min(MAX_VIEW_ROWS), "width={width} height={height}");
                    if layout.rail {
                        assert!(layout.height >= 5);
                        assert!(layout.height - 4 <= height - 4);
                    }
                }
            }
        }
        let smallest = session_composer_layout(40, 6, &multiline, &appearance);
        assert!(smallest.rail);
        assert_eq!(smallest.height, 6);
    }
}

/// Identity of a rendered message row. `row` is relative to that message's
/// actual styled output (not its body text), so cards, tool parts and group
/// separators cannot be lost during bounded-window replacement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedChatAnchor {
    pub id: String,
    pub index: usize,
    pub row: usize,
}

/// Upstream user-card left rail, pinned from `packages/tui/src/ui/border.ts`
/// (`SplitBorder.customBorderChars`): one heavy vertical stroke with EMPTY
/// caps — no top/bottom runs, no right rail, no corner glyphs. Radius
/// choices therefore have NO glyph effect on message surfaces at all; they
/// change only corners of surfaces that already draw them (composer frame).
const USER_RAIL: char = '┃';

fn boxed_message_rows(
    width: usize,
    message: &ChatMessage,
    separator: bool,
    appearance: &crate::native_box_style::BoxAppearance,
) -> Vec<StyledChatRow> {
    boxed_message_rows_spans(width, message, separator, appearance, &mut Vec::new())
}

/// Preserve the boxed rows while carrying source-derived Markdown intervals.
fn boxed_message_rows_spans(
    width: usize,
    message: &ChatMessage,
    separator: bool,
    appearance: &crate::native_box_style::BoxAppearance,
    spans_out: &mut Vec<(usize, Vec<ChatRunSpan>)>,
) -> Vec<StyledChatRow> {
    use crate::native_box_style::BoxId;
    let id = if message.role == ChatRole::Tool
        || message.parts.iter().any(|part| {
            matches!(
                part,
                ChatPart::ToolCall { .. }
                    | ChatPart::ToolResult { .. }
                    | ChatPart::ToolState { .. }
                    | ChatPart::Subtask { .. }
            )
        }) {
        BoxId::Tool
    } else {
        match message.role {
            ChatRole::User => BoxId::User,
            ChatRole::System => BoxId::System,
            ChatRole::Assistant => BoxId::Assistant,
            ChatRole::Tool => BoxId::Tool,
        }
    };
    let style = appearance.effective(id);
    // Comfortable defaults for conversation cards; an explicit per-component
    // zero still opts into compact rendering. Radius never controls spacing.
    let pad = if style.padding == 0 && appearance.overrides.get(&id).and_then(|value|value.padding).is_none()
        && matches!(id,BoxId::User|BoxId::Assistant) {1} else {usize::from(style.padding)};
    // Unstyled fallback: caller facts at the caller's wrap width, carrying
    // their full-row text interval (selected template, never decoration).
    let plain = |wrap: usize, spans_out: &mut Vec<(usize, Vec<ChatRunSpan>)>| -> Vec<StyledChatRow> {
        let mut typed_spans = Vec::new();
        let typed = typed_message_rows_with_prefix_spans(wrap.min(width), message, separator, None, &mut typed_spans);
        let rows: Vec<StyledChatRow> = typed
            .into_iter()
            .map(|(text, class)| (text, class, Some((0, width.min(u16::MAX as usize) as u16))))
            .collect();
        rebase_part_spans(typed_spans, rows.len(), 0, 0, usize::MAX, width, spans_out);
        rows
    };
    // The pinned TUI has two different primitives, not a shared padded card:
    // UserMessage is a raised left-rail surface (1 + 2 columns), whereas
    // TextPart is unboxed prose with a three-column inset. Keep explicit
    // appearance extensions below, but derive the ordinary geometry directly
    // from those primitives rather than the symmetric BoxAppearance padding.
    let default_padding = style.padding == 0
        && appearance.overrides.get(&id).and_then(|value| value.padding).is_none();
    if default_padding && style.has_border() && !appearance.full_outline(id)
        && matches!(id, BoxId::User | BoxId::Assistant) && width > 3
    {
        let inner = width - 3;
        let mut output = Vec::new();
        if separator {
            output.push((String::new(), ChatRowClass::Separator, None));
        }
        let user = id == BoxId::User;
        let padding = format!("{USER_RAIL}{}", " ".repeat(width - 1));
        if user {
            output.push((padding.clone(), ChatRowClass::UserCard, None));
        }
        let limit = MAX_MESSAGE_ROWS.saturating_sub(output.len() + usize::from(user));
        let base = output.len();
        let mut typed_spans = Vec::new();
        let typed = typed_message_rows_with_prefix_spans(inner, message, false, Some(""), &mut typed_spans);
        let taken = typed.len().min(limit);
        for (text, class) in typed.into_iter().take(limit)
        {
            let text = crate::native_view_layout::slice_cells(&text, 0, inner.min(u16::MAX as usize) as u16);
            output.push((
                format!("{}{text}", if user { "┃  " } else { "   " }),
                class,
                Some((3, inner.min(u16::MAX as usize) as u16)),
            ));
        }
        rebase_part_spans(typed_spans, taken, base, 3, inner, width, spans_out);
        if user {
            output.push((padding, ChatRowClass::UserCard, None));
        }
        return output;
    }
    // Borderless is an explicit fill/content choice. Do not enter the frame
    // path (which would reserve outline rows) and do not synthesize blank
    // border glyphs; retain the ordinary typed message classes and bounds.
    if !style.has_border() {
        return plain(width, spans_out);
    }
    // A full drawn frame exists ONLY through an explicit Double/Heavy line
    // weight (kept explicit). A radius preset or a Custom radius must never
    // create or widen a frame — changing radius affects existing corners,
    // never activates framing.
    if appearance.full_outline(id) {
        let Some(frame) = crate::native_box_style::TextFrame::new(style, width, pad) else { return plain(width, spans_out); };
        let inner = usize::from(frame.content_width());
        let mut output = Vec::new();
        if separator { output.push((String::new(), ChatRowClass::Separator, None)); }
        output.push((frame.edge(false), ChatRowClass::Separator, None));
        for _ in 0..pad { output.push((frame.blank(), ChatRowClass::Blank, None)); }
        let base = output.len();
        let mut typed_spans = Vec::new();
        let typed = typed_message_rows_with_prefix_spans(inner, message, false, None, &mut typed_spans);
        let limit = MAX_MESSAGE_ROWS.saturating_sub(2 + 2 * pad);
        let taken = typed.len().min(limit);
        for (text, class) in typed.into_iter().take(limit) {
            output.push((frame.row(&text), class, Some(((1 + pad) as u16, inner as u16))));
        }
        rebase_part_spans(typed_spans, taken, base, 1 + pad, inner, width, spans_out);
        for _ in 0..pad { output.push((frame.blank(), ChatRowClass::Blank, None)); }
        output.push((frame.edge(true), ChatRowClass::Separator, None));
        return output;
    }
    // Radius-independent per-role native identity (upstream session
    // transcript surfaces; no full rectangles anywhere):
    // * user card: upstream left rail `┃` + inset — never a frame;
    // * assistant/system: unbordered spaced prose (appearance padding act
    //   as margin rows/left margin, spacing only);
    // * tool: compact, unchanged beyond the optional margin.
    if id == BoxId::User {
        let rail = 1;
        let overhead = rail + pad;
        if width <= overhead + 1 {
            return plain(width, spans_out);
        }
        let inner = width - overhead;
        let mut output = Vec::new();
        if separator {
            output.push((String::new(), ChatRowClass::Separator, None));
        }
        let rail_pad = format!("{USER_RAIL}{}", " ".repeat(inner));
        for _ in 0..pad {
            output.push((rail_pad.clone(), ChatRowClass::Blank, None));
        }
        let base = output.len();
        let mut typed_spans = Vec::new();
        let typed = typed_message_rows_with_prefix_spans(inner, message, false, None, &mut typed_spans);
        let limit = MAX_MESSAGE_ROWS.saturating_sub(2 * pad);
        let taken = typed.len().min(limit);
        for (text, class) in typed.into_iter().take(limit)
        {
            let text =
                crate::native_view_layout::slice_cells(&text, 0, inner.min(u16::MAX as usize) as u16);
            let mut cells = 0;
            crate::native_view_layout::visit_cells(
                &text,
                inner.min(u16::MAX as usize) as u16,
                |x, _, w| cells = usize::from(x + w),
            );
            output.push((
                format!("{USER_RAIL}{}{text}", " ".repeat(pad)),
                class,
                Some(((1 + pad) as u16, inner as u16)),
            ));
        }
        rebase_part_spans(typed_spans, taken, base, 1 + pad, inner, width, spans_out);
        for _ in 0..pad {
            output.push((rail_pad.clone(), ChatRowClass::Blank, None));
        }
        return output;
    }
    if pad > 0 {
        if width <= pad.max(1) {
            return plain(width, spans_out);
        }
        let inner = width - pad;
        let mut output = Vec::new();
        if separator {
            output.push((String::new(), ChatRowClass::Separator, None));
        }
        for _ in 0..pad {
            output.push((String::new(), ChatRowClass::Blank, None));
        }
        let base = output.len();
        let mut typed_spans = Vec::new();
        let typed = typed_message_rows_with_prefix_spans(inner, message, false, None, &mut typed_spans);
        let limit = MAX_MESSAGE_ROWS.saturating_sub(2 * pad);
        let taken = typed.len().min(limit);
        for (text, class) in typed.into_iter().take(limit)
        {
            let text =
                crate::native_view_layout::slice_cells(&text, 0, inner.min(u16::MAX as usize) as u16);
            let mut cells = 0;
            crate::native_view_layout::visit_cells(
                &text,
                inner.min(u16::MAX as usize) as u16,
                |x, _, w| cells = usize::from(x + w),
            );
            output.push((
                format!("{}{text}", " ".repeat(pad)),
                class,
                Some((pad as u16, inner as u16)),
            ));
        }
        rebase_part_spans(typed_spans, taken, base, pad, inner, width, spans_out);
        for _ in 0..pad {
            output.push((String::new(), ChatRowClass::Blank, None));
        }
        output
    } else {
        plain(width, spans_out)
    }
}

/// Bounded inline label: clipped, then every control scalar (newline, ESC,
/// C0/C1) becomes a space and ANSI sequences are stripped.
fn bounded_label(text: &str) -> String {
    sanitize_inline(clip_utf8_prefix_bytes(text, MAX_LABEL_BYTES))
}

fn sanitize_inline(text: &str) -> String {
    strip_ansi(text)
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// Code sanitizer applies the same ANSI/control boundary policy as ordinary
/// display rows. Fenced tabs are expanded separately at terminal-standard
/// eight-column stops before wrapping/painting, so the row string and the
/// native paint visitor have identical cell coordinates.
fn sanitize_code(text: &str) -> String {
    strip_ansi(text)
        .chars()
        .map(|c| if c.is_control() && c != '\t' { ' ' } else { c })
        .collect()
}

fn expand_code_tabs(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut column = 0usize;
    for segment in text.split_inclusive('\t') {
        let (run, tab) = segment.strip_suffix('\t').map_or((segment, false), |s| (s, true));
        // Preserve the source-level overflow contract before the shared
        // iterator's bounded cluster projection; otherwise over-cap
        // continuations would be discarded before they can be escaped.
        let run = lossless_markdown_code_line(run);
        crate::native_view_layout::visit_unpadded_cells(&run, MAX_INPUT_ROW_BYTES, |glyph, cells| {
            if out.len().saturating_add(glyph.len()) <= MAX_INPUT_ROW_BYTES {
                out.push_str(glyph);
                column = column.saturating_add(cells);
            }
        });
        if tab {
            let count = 8 - (column % 8);
            if out.len().saturating_add(count) > MAX_INPUT_ROW_BYTES { break; }
            out.extend(std::iter::repeat_n(' ', count));
            column = column.saturating_add(count);
        }
    }
    out
}

/// Left padding for message text of `role`. User (and tool-role) bodies are
/// indented as the upstream marker block; assistant/system text is flush.
fn text_prefix(role: ChatRole) -> &'static str {
    match role {
        ChatRole::User | ChatRole::Tool => "  ",
        ChatRole::Assistant | ChatRole::System => "",
    }
}

/// True when message `index` starts a new display group: every user message,
/// and the first system message of a run. Assistant/tool messages continue
/// the surrounding block.
fn starts_new_group(index: usize, messages: &[ChatMessage]) -> bool {
    if index == 0 {
        return false;
    }
    match messages[index].role {
        ChatRole::User => true,
        ChatRole::System => messages[index - 1].role != ChatRole::System,
        ChatRole::Assistant | ChatRole::Tool => false,
    }
}

// SessionRowView gives an assistant row its own one-row top margin. Keep
// the legacy plain-text transcript grouping separate from the styled TUI.
fn starts_styled_group(index: usize, messages: &[ChatMessage]) -> bool {
    starts_new_group(index, messages)
        || (index > 0 && messages[index].role == ChatRole::Assistant)
}

/// `[tool] name (id)` / `[result] name` / `[error] name` header, from
/// caller-supplied facts only; an absent id renders no parenthesized part.
fn tool_header(kind: &str, name: &str, id: &str) -> String {
    let name = bounded_label(name);
    let id = bounded_label(id);
    if id.is_empty() {
        format!("  [{kind}] {name}")
    } else {
        format!("  [{kind}] {name} ({id})")
    }
}

/// Wrap one part's text with `prefix` and push `class` rows, bounded to the
/// per-message row cap. Input bytes are read as a bounded UTF-8 prefix and
/// sanitized; `width == 0` keeps the existing no-wrap behavior.
fn push_wrapped(
    rows: &mut Vec<(String, ChatRowClass)>,
    width: usize,
    text: &str,
    prefix: &str,
    class: ChatRowClass,
) -> bool {
    let clean = sanitize(clip_utf8_prefix_bytes(text, MAX_INPUT_ROW_BYTES));
    let content_width = if width == 0 {
        0
    } else {
        width.saturating_sub(prefix.len()).max(1)
    };
    for line in clean.split('\n') {
        for row in render_lines(
            &[TranscriptLine {
                id: 0,
                text: line.to_owned(),
            }],
            content_width,
        ) {
            if rows.len() >= MAX_MESSAGE_ROWS {
                return true;
            }
            rows.push((truncate_row(&format!("{prefix}{row}"), width), class));
        }
    }
    false
}

/// Preserve fenced-code whitespace while wrapping. Prose keeps the existing
/// word-wrap behavior; fenced rows are split only at display-cluster
/// boundaries so indentation and repeated spaces remain actual source text.
fn push_markdown_wrapped(
    rows: &mut Vec<(String, ChatRowClass)>,
    width: usize,
    text: &str,
    prefix: &str,
    class: ChatRowClass,
) {
    let input_truncated = text.len() > MAX_INPUT_ROW_BYTES;
    // Keep the bounded source raw until fence state is known: code indentation
    // is source data, so the ordinary tab-normalizing sanitizer must not run
    // before fenced lines are dispatched.
    let bounded = clip_utf8_prefix_bytes(text, MAX_INPUT_ROW_BYTES);
    // Strip escape sequences over the complete bounded message before line
    // projection. This preserves ANSI state that legally spans a newline;
    // per-line stripping would expose payload bytes from an unterminated OSC,
    // DCS, or CSI prefix.
    let sanitized = strip_ansi(bounded);
    let mut fence: Option<(char, usize)> = None;
    let mut overflow = input_truncated;
    for raw_line in sanitized.split('\n') {
        if rows.len() >= MAX_MESSAGE_ROWS {
            overflow = true;
            break;
        }
        let was_fenced = fence.is_some();
        let mut code = was_fenced;
        if let Some((marker, run, trailing)) = markdown_fence_marker(raw_line) {
            match fence {
                Some((open, minimum))
                    if marker == open
                        && run >= minimum
                        && trailing.bytes().all(|byte| matches!(byte, b' ' | b'\t')) =>
                {
                    fence = None;
                    code = true;
                }
                None => {
                    fence = Some((marker, run));
                    code = true;
                }
                _ => {}
            }
        }
        let line = if code {
            expand_code_tabs(&sanitize_code(raw_line))
        } else {
            sanitize(raw_line)
        };
        let line_overflow = if code {
            push_code_wrapped(rows, width, &line, prefix, class)
        } else {
            push_wrapped(rows, width, &line, prefix, class)
        };
        if line_overflow {
            overflow = true;
            break;
        }
    }
    if overflow {
        push_markdown_overflow(rows, width, prefix, class);
    }
}

/// Recognize a CommonMark-style opening/closing fence with up to three
/// leading spaces. Returns its marker, run length and trailing info string.
fn markdown_fence_marker(line: &str) -> Option<(char, usize, &str)> {
    let indent = line.as_bytes().iter().take_while(|byte| **byte == b' ').count();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let marker = rest.chars().next()?;
    if !matches!(marker, '`' | '~') {
        return None;
    }
    let run = rest.bytes().take_while(|byte| *byte == marker as u8).count();
    if run < 3 {
        return None;
    }
    let trailing = &rest[run..];
    if marker == '`' && trailing.contains('`') {
        return None;
    }
    Some((marker, run, trailing))
}

/// Mirrors the shared display helper's continuation ranges and cluster caps.
/// ponytail: mirror the 8-continuation/4-base/48-byte ceiling until the shared helper path is granted; move this detector there.
fn markdown_code_continuation(c: char) -> bool {
    const RANGES: &[(u32, u32)] = &[
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
    let cp = c as u32;
    RANGES.iter().any(|&(start, end)| (start..=end).contains(&cp))
        || (0x1F3FB..=0x1F3FF).contains(&cp)
}

/// Preserve ordinary code clusters verbatim. If the shared bounded cluster
/// limits would discard part of one cluster, escape that cluster only; do not
/// make an unrelated following scalar lossy merely because it shares a line.
fn lossless_markdown_code_line(text: &str) -> String {
    fn escaped(cluster: &str) -> String {
        cluster.chars().map(|c| {
            if c.is_ascii() { c.to_string() } else { format!("\\u{{{:x}}}", c as u32) }
        }).collect()
    }
    let mut out = String::with_capacity(text.len());
    let mut cluster = String::new();
    let mut continuations = 0usize;
    let mut bases = 1usize;
    let mut bytes = 0usize;
    let mut after_join = false;
    let mut regional_count = 0usize;
    let mut bad = false;
    let mut flush = |out: &mut String, cluster: &mut String, bad: &mut bool| {
        if *bad { out.push_str(&escaped(cluster)); } else { out.push_str(cluster); }
        cluster.clear();
        *bad = false;
    };
    for c in text.chars() {
        let regional = (0x1F1E6..=0x1F1FF).contains(&(c as u32));
        if regional {
            if regional_count == 1 && !cluster.is_empty() {
                cluster.push(c);
                regional_count = 2;
                continue;
            }
            if !cluster.is_empty() {
                if regional_count == 1 { bad = true; }
                flush(&mut out, &mut cluster, &mut bad);
            }
            cluster.push(c);
            regional_count = 1;
            bytes = c.len_utf8();
            continuations = 0;
            bases = 1;
            after_join = false;
            continue;
        }
        if regional_count == 1 {
            bad = true;
            flush(&mut out, &mut cluster, &mut bad);
            regional_count = 0;
        } else if regional_count == 2 {
            regional_count = 0;
        }
        if markdown_code_continuation(c) {
            if cluster.is_empty() {
                out.push_str(&escaped(&c.to_string()));
                continue;
            }
            cluster.push(c);
            continuations += 1;
            bytes += c.len_utf8();
            bad |= continuations > 8 || bytes > 48;
            after_join = c == '\u{200d}';
            continue;
        }
        if cluster.is_empty() {
            cluster.push(c);
            bytes = c.len_utf8();
            continuations = 0;
            bases = 1;
            after_join = false;
            continue;
        }
        if after_join {
            cluster.push(c);
            bases += 1;
            bytes += c.len_utf8();
            bad |= bases > 4 || bytes > 48;
            after_join = false;
        } else {
            flush(&mut out, &mut cluster, &mut bad);
            cluster.push(c);
            bytes = c.len_utf8();
            continuations = 0;
            bases = 1;
        }
    }
    if regional_count == 1 { bad = true; }
    flush(&mut out, &mut cluster, &mut bad);
    out
}

/// Emit a bounded marker when an input-byte or row ceiling hides source text.
/// Replacing the final row preserves the existing row cap.
fn push_markdown_overflow(
    rows: &mut Vec<(String, ChatRowClass)>,
    width: usize,
    prefix: &str,
    class: ChatRowClass,
) {
    let marker_prefix = if width == 0 || prefix.len() < width { prefix } else { "" };
    let marker = if width == 0 {
        format!("{marker_prefix}[display truncated]")
    } else {
        format!("{marker_prefix}! content omitted")
    };
    let marker = truncate_row(&marker, width);
    if rows.len() >= MAX_MESSAGE_ROWS {
        if let Some(last) = rows.last_mut() {
            *last = (marker, class);
        }
    } else {
        rows.push((marker, class));
    }
}

/// Code-row wrapper that keeps bounded source content and whitespace. A wide
/// scalar that cannot fit an exceptionally narrow pane uses a Unicode escape.
fn push_code_wrapped(
    rows: &mut Vec<(String, ChatRowClass)>,
    width: usize,
    text: &str,
    prefix: &str,
    class: ChatRowClass,
) -> bool {
    if rows.len() >= MAX_MESSAGE_ROWS {
        return true;
    }
    let clean = sanitize_code(clip_utf8_prefix_bytes(text, MAX_INPUT_ROW_BYTES));
    if width == 0 {
        rows.push((format!("{prefix}{clean}"), class));
        return false;
    }
    let clean = lossless_markdown_code_line(&clean);
    // Decoration yields to message content on a pane too narrow to fit both.
    let prefix = if prefix.len() < width { prefix } else { "" };
    let content_width = width.saturating_sub(prefix.len()).max(1);
    if clean.is_empty() {
        rows.push((truncate_row(prefix, width), class));
        return false;
    }
    let mut clusters = Vec::new();
    crate::native_view_layout::visit_unpadded_cells(
        &clean,
        clean.len(),
        |glyph, cells| clusters.push((glyph.to_owned(), cells)),
    );
    let mut line = String::new();
    let mut used = 0usize;
    for (glyph, cells) in clusters {
        if rows.len() >= MAX_MESSAGE_ROWS {
            return true;
        }
        if cells > content_width {
            if !line.is_empty() {
                rows.push((truncate_row(&format!("{prefix}{line}"), width), class));
                line.clear();
                used = 0;
                if rows.len() >= MAX_MESSAGE_ROWS {
                    return true;
                }
            }
            let escaped: String = glyph.chars().map(|c| format!("\\u{{{:x}}}", c as u32)).collect();
            for chunk in escaped.chars().collect::<Vec<_>>().chunks(content_width) {
                if rows.len() >= MAX_MESSAGE_ROWS { return true; }
                let chunk: String = chunk.iter().collect();
                rows.push((truncate_row(&format!("{prefix}{chunk}"), width), class));
            }
            continue;
        }
        if used + cells > content_width {
            rows.push((truncate_row(&format!("{prefix}{line}"), width), class));
            line.clear();
            used = 0;
            if rows.len() >= MAX_MESSAGE_ROWS {
                return true;
            }
        }
        line.push_str(&glyph);
        used += cells;
    }
    if !line.is_empty() && rows.len() < MAX_MESSAGE_ROWS {
        rows.push((truncate_row(&format!("{prefix}{line}"), width), class));
    }
    false
}

/// Source-derived style interval in display cells; link targets are not actionable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatRunSpan {
    pub start: u16,
    pub end: u16,
    pub flags: u8,
}

fn row_cells(text: &str) -> usize {
    crate::native_view_layout::visit_unpadded_cells(text, text.len(), |_, _| {})
}

/// Reproduce the existing wrapper's geometry, accepting style only from the
/// undecorated canonical part. Any inconsistent projection drops its spans.
fn markdown_part_row_spans(
    text: &str,
    width: usize,
    prefix: &str,
    actual: &[(String, ChatRowClass)],
) -> Vec<Vec<ChatRunSpan>> {
    let parsed = crate::markdown_runs::parse_part(crate::markdown_runs::PartKind::Prose, text);
    let prefix_cells = row_cells(prefix);
    let mut out: Vec<(Vec<ChatRunSpan>, Option<usize>)> = Vec::new();
    let bounded = clip_utf8_prefix_bytes(text, MAX_INPUT_ROW_BYTES);
    let sanitized = strip_ansi(bounded);
    let mut fence: Option<(char, usize)> = None;
    let mut overflow = text.len() > MAX_INPUT_ROW_BYTES;
    let mut dropped = false;
    for (index, raw_line) in sanitized.split('\n').enumerate() {
        if out.len() >= MAX_MESSAGE_ROWS { overflow = true; break; }
        let mut code = fence.is_some();
        if let Some((marker, run, trailing)) = markdown_fence_marker(raw_line) {
            match fence {
                Some((open, minimum)) if marker == open && run >= minimum
                    && trailing.bytes().all(|byte| matches!(byte, b' ' | b'\t')) => {
                    fence = None;
                    code = true;
                }
                None => { fence = Some((marker, run)); code = true; }
                _ => {}
            }
        }
        let Some(source_row) = parsed.rows.get(index) else { return Vec::new(); };
        if source_row.code != code { return Vec::new(); }
        let line = if code { expand_code_tabs(&sanitize_code(raw_line)) } else { sanitize(raw_line) };
        let mut capped = false;
        if code {
            let prefix_eff = if width == 0 || prefix.len() < width { prefix } else { "" };
            for (chunk, cells) in code_row_chunks(&line, width, prefix) {
                if out.len() >= MAX_MESSAGE_ROWS { capped = true; break; }
                let start = row_cells(prefix_eff);
                let mut end = start.saturating_add(cells);
                let row = truncate_row(&format!("{prefix_eff}{chunk}"), width);
                let rendered_cells = row_cells(&row);
                if width > 0 { end = end.min(rendered_cells); }
                let spans = if end > start {
                    vec![ChatRunSpan { start: start as u16, end: end as u16,
                        flags: crate::markdown_runs::CODE | crate::markdown_runs::CODE_BLOCK }]
                } else { Vec::new() };
                out.push((spans, Some(rendered_cells)));
            }
        } else {
            let content_width = if width == 0 { 0 } else { width.saturating_sub(prefix.len()).max(1) };
            let clean = sanitize(clip_utf8_prefix_bytes(&line, MAX_INPUT_ROW_BYTES));
            let mut cursor = 0usize;
            for fragment in render_lines(&[TranscriptLine { id: 0, text: clean.clone() }], content_width) {
                if out.len() >= MAX_MESSAGE_ROWS { capped = true; break; }
                let mut spans = Vec::new();
                let row = truncate_row(&format!("{prefix}{fragment}"), width);
                let rendered_cells = row_cells(&row);
                if !dropped {
                    match map_fragment_chars(&source_row.text, cursor, &fragment) {
                        Some((pairs, next_cursor)) => {
                            cursor = next_cursor;
                            spans = project_pairs(&source_row.runs, &fragment, &pairs, prefix_cells, rendered_cells);
                        }
                        None => { dropped = true; }
                    }
                }
                out.push((spans, Some(rendered_cells)));
            }
        }
        if capped { overflow = true; break; }
    }
    if overflow {
        if out.len() >= MAX_MESSAGE_ROWS {
            if let Some(last) = out.last_mut() { last.0.clear(); last.1 = None; }
        } else { out.push((Vec::new(), None)); }
    }
    if dropped || out.len() != actual.len() { return Vec::new(); }
    for (index, (_, expected)) in out.iter().enumerate() {
        if let Some(expected) = expected {
            if row_cells(&actual[index].0) != *expected { return Vec::new(); }
        }
    }
    out.into_iter().map(|(spans, _)| spans).collect()
}

fn code_row_chunks(line: &str, width: usize, prefix: &str) -> Vec<(String, usize)> {
    let clean = sanitize_code(clip_utf8_prefix_bytes(line, MAX_INPUT_ROW_BYTES));
    if width == 0 { return vec![(clean.clone(), row_cells(&clean))]; }
    let clean = lossless_markdown_code_line(&clean);
    let prefix_eff_len = if prefix.len() < width { prefix.len() } else { 0 };
    let content_width = width.saturating_sub(prefix_eff_len).max(1);
    if clean.is_empty() { return vec![(String::new(), 0)]; }
    let mut clusters: Vec<(String, usize)> = Vec::new();
    crate::native_view_layout::visit_unpadded_cells(&clean, clean.len(), |glyph, cells| {
        clusters.push((glyph.to_owned(), cells));
    });
    let mut out = Vec::new();
    let mut used = 0usize;
    let mut line_text = String::new();
    for (glyph, cells) in clusters {
        if cells > content_width {
            if !line_text.is_empty() { out.push((std::mem::take(&mut line_text), std::mem::take(&mut used))); }
            let escaped: String = glyph.chars().map(|c| format!("\\u{{{:x}}}", c as u32)).collect();
            for chunk in escaped.chars().collect::<Vec<_>>().chunks(content_width) {
                let chunk: String = chunk.iter().collect();
                out.push((chunk.clone(), chunk.chars().count()));
            }
            continue;
        }
        if used + cells > content_width { out.push((std::mem::take(&mut line_text), std::mem::take(&mut used))); }
        line_text.push_str(&glyph);
        used += cells;
    }
    if !line_text.is_empty() { out.push((line_text, used)); }
    out
}

/// Explicit mapping from sanitized source bytes to wrap-fragment bytes.
/// Whitespace is consumed separately because prose wrapping collapses it.
fn map_fragment_chars(source: &str, cursor: usize, fragment: &str) -> Option<(Vec<(usize, usize, usize)>, usize)> {
    let mut pairs = Vec::new();
    let mut source_at = cursor.min(source.len());
    let mut frag_at = 0usize;
    loop {
        while let Some(ch) = source[source_at..].chars().next() {
            if !ch.is_whitespace() { break; }
            source_at += ch.len_utf8();
        }
        while let Some(ch) = fragment[frag_at..].chars().next() {
            if !ch.is_whitespace() { break; }
            frag_at += ch.len_utf8();
        }
        if frag_at >= fragment.len() { return Some((pairs, source_at)); }
        if source_at >= source.len() { return None; }
        let source_ch = source[source_at..].chars().next()?;
        let frag_ch = fragment[frag_at..].chars().next()?;
        if source_ch != frag_ch { return None; }
        pairs.push((source_at, source_at + source_ch.len_utf8(), frag_at));
        source_at += source_ch.len_utf8();
        frag_at += frag_ch.len_utf8();
    }
}

/// A style is projected only when all characters of a display cluster map
/// inside a source run. Decorations and link targets never create style facts.
fn project_pairs(
    runs: &[crate::markdown_runs::MarkdownRun], fragment: &str,
    pairs: &[(usize, usize, usize)], prefix_cells: usize, row_end: usize,
) -> Vec<ChatRunSpan> {
    if runs.is_empty() || pairs.is_empty() { return Vec::new(); }
    let mut spans: Vec<ChatRunSpan> = Vec::new();
    let mut pair_at = 0usize;
    let mut frag_byte = 0usize;
    let mut cell = 0usize;
    crate::native_view_layout::visit_unpadded_cells(fragment, fragment.len(), |glyph, cells| {
        frag_byte += glyph.len();
        let cell_start = cell;
        cell += cells;
        let cluster_chars = glyph.chars().count();
        let mut mapped = 0usize;
        let mut source_start = usize::MAX;
        let mut source_end = 0usize;
        while pair_at < pairs.len() && pairs[pair_at].2 < frag_byte {
            let (start, end, _) = pairs[pair_at];
            source_start = source_start.min(start);
            source_end = source_end.max(end);
            mapped += 1;
            pair_at += 1;
        }
        if mapped != cluster_chars || mapped == 0 { return; }
        let mut flags = 0u8;
        for run in runs.iter().take(crate::markdown_runs::MAX_RUNS_PER_ROW) {
            if run.start <= source_start && run.end >= source_end { flags |= run.flags; }
        }
        if flags == 0 { return; }
        let start = prefix_cells.saturating_add(cell_start);
        let end = prefix_cells.saturating_add(cell).min(row_end);
        if start >= end { return; }
        if let Some(last) = spans.last_mut() {
            if last.flags == flags && usize::from(last.end) == start { last.end = end as u16; return; }
        }
        spans.push(ChatRunSpan { start: start as u16, end: end as u16, flags });
    });
    spans
}

fn rebase_part_spans(
    spans: Vec<(usize, Vec<ChatRunSpan>)>, taken: usize, base: usize,
    left: usize, inner: usize, width: usize, sink: &mut Vec<(usize, Vec<ChatRunSpan>)>,
) {
    for (index, row_spans) in spans {
        if index >= taken || row_spans.is_empty() { continue; }
        let rebased: Vec<ChatRunSpan> = row_spans.into_iter().filter_map(|span| {
            let start = usize::from(span.start).min(inner).saturating_add(left);
            let mut end = usize::from(span.end).min(inner).saturating_add(left);
            if width > 0 { end = end.min(width); }
            (start < end).then_some(ChatRunSpan { start: start as u16, end: end as u16, flags: span.flags })
        }).collect();
        if !rebased.is_empty() { sink.push((base + index, rebased)); }
    }
}

/// Display rows of one typed message: optional group separator, optional
/// agent/model metadata row, then its parts in order. Tool results are
/// labeled `[result]`/`[error]` with indented output — the view never
/// executes, simulates or re-parses a tool.
fn typed_message_rows(
    width: usize,
    message: &ChatMessage,
    separator: bool,
) -> Vec<(String, ChatRowClass)> {
    typed_message_rows_with_prefix(width, message, separator, None)
}

fn typed_message_rows_with_prefix(
    width: usize,
    message: &ChatMessage,
    separator: bool,
    prefix: Option<&str>,
) -> Vec<(String, ChatRowClass)> {
    typed_message_rows_with_prefix_spans(width, message, separator, prefix, &mut Vec::new())
}

fn typed_message_rows_with_prefix_spans(
    width: usize, message: &ChatMessage, separator: bool, prefix: Option<&str>,
    spans_out: &mut Vec<(usize, Vec<ChatRunSpan>)>,
) -> Vec<(String, ChatRowClass)> {
    let mut rows: Vec<(String, ChatRowClass)> = Vec::new();
    if separator {
        rows.push((String::new(), ChatRowClass::Separator));
    }
    // Upstream `AssistantFooter` IS the metadata line. When real execution
    // facts are present the footer renders the agent/model/interrupted spans
    // itself, so the legacy `agent: … model: …` row is suppressed to avoid
    // double metadata; fact-less messages keep the legacy row unchanged.
    if message.assistant_facts.is_none() {
        if let Some(metadata) = message.metadata_row() {
            if rows.len() < MAX_MESSAGE_ROWS {
                rows.push((truncate_row(&metadata, width), ChatRowClass::Metadata));
            }
        }
    }
    for part in message.parts.iter().take(MAX_PARTS_PER_MESSAGE) {
        if rows.len() >= MAX_MESSAGE_ROWS {
            break;
        }
        match part {
            ChatPart::Text(text) => {
                let push = |rows: &mut Vec<(String, ChatRowClass)>, width, text: &str, prefix: &str, class| {
                    if message.role == ChatRole::User {
                        push_wrapped(rows, width, text, prefix, class);
                    } else {
                        push_markdown_wrapped(rows, width, text, prefix, class);
                    }
                };
                let part_prefix = prefix.unwrap_or_else(|| text_prefix(message.role));
                let before = rows.len();
                push(
                    &mut rows,
                    width,
                    text,
                    part_prefix,
                    match message.role {
                        ChatRole::User => ChatRowClass::UserCard,
                        ChatRole::Assistant => ChatRowClass::AssistantCard,
                        ChatRole::System => ChatRowClass::SystemCard,
                        ChatRole::Tool => ChatRowClass::ToolResult,
                    },
                );
                if message.role != ChatRole::User {
                    for (offset, spans) in markdown_part_row_spans(text, width, part_prefix, &rows[before..]).into_iter().enumerate() {
                        if !spans.is_empty() { spans_out.push((before + offset, spans)); }
                    }
                }
            }
            ChatPart::Thinking(text) => {
                // Match the pinned TUI's reasoning presentation: providers can
                // mark encrypted/unavailable reasoning with this literal token;
                // it is a placeholder, not user-facing reasoning content.
                // Bound the input before replacement so this transformation
                // cannot allocate in proportion to an untrusted provider string.
                let text = clip_utf8_prefix_bytes(text, MAX_INPUT_ROW_BYTES);
                let display = text.replace("[REDACTED]", "");
                let display = display.trim();
                if !display.is_empty() {
                    push_wrapped(&mut rows, width, display, "  ", ChatRowClass::Thinking);
                }
            }
            ChatPart::ToolCall { name, id } => {
                rows.push((
                    truncate_row(&tool_header("tool", name, id), width),
                    ChatRowClass::ToolCall,
                ));
            }
            ChatPart::ToolResult {
                name,
                output,
                is_error,
            } => {
                let (kind, header_class, body_class) = if *is_error {
                    ("error", ChatRowClass::ToolError, ChatRowClass::ToolError)
                } else {
                    ("result", ChatRowClass::ToolResult, ChatRowClass::ToolResult)
                };
                rows.push((
                    truncate_row(&tool_header(kind, name, ""), width),
                    header_class,
                ));
                push_wrapped(&mut rows, width, output, "    ", body_class);
            }
            ChatPart::ToolState {
                name,
                id,
                status,
                args,
                output,
                is_error,
            } => {
                let class = if *is_error {
                    ChatRowClass::ToolError
                } else {
                    ChatRowClass::ToolCall
                };
                rows.push((truncate_row(&tool_header(status, name, id), width), class));
                if !args.is_empty() {
                    push_wrapped(&mut rows, width, args, "    ", class);
                }
                if !output.is_empty() {
                    push_wrapped(&mut rows, width, output, "    ", class);
                }
            }
            ChatPart::File { filename, mime } => {
                push_wrapped(
                    &mut rows,
                    width,
                    &format!("[file] {filename} ({mime})"),
                    "  ",
                    ChatRowClass::Metadata,
                );
            }
            ChatPart::Subtask {
                agent,
                description,
                background,
            } => {
                push_wrapped(
                    &mut rows,
                    width,
                    &format!(
                        "[task] {agent}{} — {description}",
                        if *background { " (background)" } else { "" }
                    ),
                    "  ",
                    ChatRowClass::ToolCall,
                );
            }
            ChatPart::Retry { attempt, message } => {
                push_wrapped(
                    &mut rows,
                    width,
                    &format!("[retry {attempt}] {message}"),
                    "  ",
                    ChatRowClass::ToolError,
                );
            }
            ChatPart::Step { kind, detail } => {
                push_wrapped(
                    &mut rows,
                    width,
                    &format!("[{kind}] {detail}"),
                    "  ",
                    ChatRowClass::Metadata,
                );
            }
            ChatPart::Compaction { reason } => {
                push_wrapped(
                    &mut rows,
                    width,
                    &format!("[compaction] {reason}"),
                    "  ",
                    ChatRowClass::Metadata,
                );
            }
            ChatPart::Patch { path, diff } => {
                rows.push((
                    truncate_row(&format!("[diff] {path}"), width),
                    ChatRowClass::Metadata,
                ));
                // ONE parser pass at the parser's own bounded ceiling: exact
                // malformed/truncated facts plus all rows; message-budget
                // overflow is computed here and OR-ed into the truncation
                // fact. Never a second parse.
                // The four-cell message inset is outside the diff surface;
                // wrap against the measured remaining pane width so wide and
                // combining Unicode clusters are not split by char count.
                let rendered = crate::native_diff_view::paint_rows_with_gutter_wrapped(
                    diff,
                    crate::native_diff_view::MAX_DIFF_LINES,
                    width.saturating_sub(4),
                );
                let total_rows = rendered.rows.len();
                let malformed = rendered.malformed;
                // Reserve real status row(s) BEFORE the parser row budget:
                // each true fact gets its own distinct row inside the hard
                // MAX_MESSAGE_ROWS cap (the frozen regressions require both
                // substrings to survive when both facts are true).
                let budget = MAX_MESSAGE_ROWS.saturating_sub(rows.len());
                let malformed_slots = usize::from(malformed);
                let kept_without_trunc_status = total_rows.min(budget.saturating_sub(malformed_slots));
                let truncated = rendered.truncated || kept_without_trunc_status < total_rows;
                let reserve = malformed_slots + usize::from(truncated);
                let kept = total_rows.min(budget.saturating_sub(reserve));
                for line in rendered.rows.into_iter().take(kept) {
                    let class = match line.role {
                        crate::native_diff_view::DiffLineRole::Added => ChatRowClass::DiffAdded,
                        crate::native_diff_view::DiffLineRole::Removed => ChatRowClass::DiffRemoved,
                        _ => ChatRowClass::Metadata,
                    };
                    rows.push((truncate_row(&format!("    {}", line.text), width), class));
                }
                let mut statuses: Vec<(String, ChatRowClass)> = Vec::new();
                if malformed {
                    statuses.push((
                        truncate_row("    [malformed diff]", width),
                        ChatRowClass::ToolError,
                    ));
                }
                if truncated {
                    statuses.push((
                        truncate_row("    [diff truncated]", width),
                        ChatRowClass::Metadata,
                    ));
                }
                // Place reserved status rows as DISTINCT rows. Normally the
                // reservation left slack; in the pathological case (earlier
                // parts already filled the cap) free exactly `statuses.len()`
                // tail rows in ONE truncate so later statuses can never
                // overwrite earlier ones, then append all of them.
                if !statuses.is_empty() {
                    let slack = MAX_MESSAGE_ROWS.saturating_sub(rows.len());
                    if statuses.len() > slack {
                        rows.truncate(rows.len().saturating_sub(statuses.len() - slack));
                    }
                    rows.extend(statuses);
                }
            }
        }
    }
    // Assistant completion footer (upstream `AssistantFooter`): appended after
    // the message parts, only when real execution facts exist. Missing facts
    // render nothing. Footer rows receive the same message prefix/inset as
    // ordinary prose rows (upstream's footer `paddingLeft={3}` and `TextPart`
    // inset are the same three columns).
    if let Some(footer) = crate::native_footer::render_assistant_footer(message, width) {
        let prefix = prefix.unwrap_or_else(|| text_prefix(message.role));
        for (content, class) in footer.rows {
            if rows.len() >= MAX_MESSAGE_ROWS {
                break;
            }
            if content.is_empty() {
                rows.push((String::new(), class));
            } else {
                rows.push((truncate_row(&format!("{prefix}{content}"), width), class));
            }
        }
    }
    rows
}

#[cfg(test)]
mod diff_wiring_v2_regressions {
    use super::*;

    fn patch_message(diff: String) -> ChatMessage {
        ChatMessage { role: ChatRole::Assistant, parts: vec![ChatPart::Patch { path: "many.rs".into(), diff }], agent: None, model: None, assistant_facts: None }
    }

    #[test]
    fn status_survives_full_message_budget_and_is_truthful() {
        // One header plus a valid hunk exceeding the render budget.
        let valid = format!("@@ -1,{} +1,{} @@\n{}", MAX_MESSAGE_ROWS + 4, MAX_MESSAGE_ROWS + 4,
            (0..MAX_MESSAGE_ROWS + 4).map(|_| " same\n").collect::<String>());
        let rows = typed_message_rows(100, &patch_message(valid), false);
        assert_eq!(rows.len(), MAX_MESSAGE_ROWS);
        assert!(rows.iter().any(|(row, _)| row.contains("[diff truncated]")));
        assert!(!rows.iter().any(|(row, _)| row.contains("[malformed diff]")));

        // Malformed count and budget overflow both remain visibly reported.
        let malformed = format!("@@ -1,{} +1,{} @@\n{}", MAX_MESSAGE_ROWS + 4, MAX_MESSAGE_ROWS + 4,
            (0..MAX_MESSAGE_ROWS + 4).map(|_| "+ extra\n").collect::<String>());
        let rows = typed_message_rows(100, &patch_message(malformed), false);
        assert_eq!(rows.len(), MAX_MESSAGE_ROWS);
        assert!(rows.iter().any(|(row, class)| *class == ChatRowClass::ToolError && row.contains("[malformed diff]")));
        assert!(rows.iter().any(|(row, _)| row.contains("[diff truncated]")));
    }

    #[test]
    fn narrow_unicode_output_remains_bounded_and_status_is_visible() {
        let diff = format!("@@ -1,{} +1,{} @@\n{}", MAX_MESSAGE_ROWS + 2, MAX_MESSAGE_ROWS + 2,
            (0..MAX_MESSAGE_ROWS + 2).map(|_| "+ 🦀\n").collect::<String>());
        let rows = typed_message_rows(24, &patch_message(diff), false);
        assert_eq!(rows.len(), MAX_MESSAGE_ROWS);
        assert!(rows.iter().all(|(row, _)| row.chars().count() <= 24));
        assert!(rows.iter().any(|(row, _)| row.contains("[diff truncated]")));
    }
}

impl ChatView {
    /// Typed counterpart of [`ChatView::render`]: same bounded frame
    /// (workspace header, bottom-anchored transcript window ending at the
    /// tail, composer on the last rows), but the transcript is built from
    /// caller-typed [`ChatMessage`] roles/parts. Roles are never inferred
    /// from rendered text.
    #[must_use]
    pub fn render_typed(
        &self,
        width: usize,
        height: usize,
        workspace: &str,
        messages: &[ChatMessage],
        draft: &str,
        busy: bool,
    ) -> Vec<String> {
        self.render_typed_classified(width, height, workspace, messages, draft, busy)
            .into_iter()
            .map(|(row, _)| row)
            .collect()
    }

    /// Same frame as [`ChatView::render_typed`] with each row's display
    /// class retained for the paint layer.
    #[must_use]
    pub fn render_typed_classified(
        &self,
        width: usize,
        height: usize,
        workspace: &str,
        messages: &[ChatMessage],
        draft: &str,
        busy: bool,
    ) -> Vec<(String, ChatRowClass)> {
        let height = height.min(MAX_VIEW_ROWS);
        if height == 0 {
            return Vec::new();
        }
        let draft = clip_utf8_prefix_bytes(draft, MAX_DRAFT_BYTES);
        let draft_lines = draft.split('\n').count();
        let composer_h = draft_lines.clamp(1, (height / 2).max(1));
        let header_h = usize::from(height > composer_h);
        let transcript_h = height - composer_h - header_h;

        let mut rows: Vec<(String, ChatRowClass)> = Vec::with_capacity(height);
        let window = self.typed_window(width, transcript_h, messages);
        if header_h == 1 {
            rows.push((
                render_header(width, workspace, busy, self.scroll.get()),
                ChatRowClass::Header,
            ));
        }
        for _ in 0..transcript_h.saturating_sub(window.len()) {
            rows.push((String::new(), ChatRowClass::Blank));
        }
        rows.extend(window);
        let composer = bounded_composer_rows(draft, composer_h + 1);
        for row in render_composer(width, composer_h, &composer.rows, composer.total_lines) {
            rows.push((row, ChatRowClass::Composer));
        }
        rows.truncate(height);
        rows
    }

    /// Visible typed rows: the `height` rows ending `scroll` rows above the
    /// tail. Walks messages backwards from the tail and stops as soon as
    /// `scroll + height` rows exist; only the touched messages are rendered
    /// (each bounded by [`MAX_MESSAGE_ROWS`]) — following the tail never
    /// materializes the whole history.
    /// Typed rows with the composer drawn as a real bordered frame.
    ///
    /// Same header/transcript/composer budget as
    /// [`ChatView::render_typed_classified`], but the composer region
    /// reserves two border rows plus the effective inner padding, and the
    /// draft is painted INSIDE the border (never over it). Narrow panes
    /// fall back to the legacy unframed prompt.
    #[must_use]
    pub fn render_typed_classified_framed(
        &self,
        width: usize,
        height: usize,
        workspace: &str,
        messages: &[ChatMessage],
        draft: &str,
        busy: bool,
        frame: crate::native_box_style::EffectiveBox,
    ) -> Vec<(String, ChatRowClass)> {
        // Bound the direct framed API before any layout decision or capacity
        // allocation. The ordinary renderer has the same public-input bound.
        let height = height.min(MAX_VIEW_ROWS);
        // Borderless is a fill/content surface, not a degenerate framed
        // surface. Reuse the ordinary bounded composer layout so no outline
        // rows, corner glyphs, or reserved frame space are fabricated.
        if !frame.has_border() {
            return self.render_typed_classified(width, height, workspace, messages, draft, busy);
        }
        let layout = composer_layout(width, height, draft, frame);
        if !layout.framed {
            return self.render_typed_classified(width, height, workspace, messages, draft, busy);
        }
        let pad = usize::from(frame.padding);
        let overhead = 2 + 2 * pad;
        let frame_h = layout.height;
        let body_h = height - frame_h;
        let content_h = frame_h - overhead;
        let content_w = width - 2 - 2 * pad;
        let header_h = usize::from(body_h > 0);
        let window = self.typed_window(width, body_h - header_h, messages);
        let mut rows = Vec::with_capacity(height);
        while rows.len() < body_h.saturating_sub(header_h).saturating_sub(window.len()) {
            rows.push((String::new(), ChatRowClass::Blank));
        }
        rows.extend(window);
        if header_h > 0 {
            rows.insert(
                0,
                (render_header(width, workspace, busy, self.scroll.get()), ChatRowClass::Header),
            );
        }
        let draft = clip_utf8_prefix_bytes(draft, MAX_DRAFT_BYTES);
        let input = crate::native_prompt_layout::wrap_prompt(draft, content_w.saturating_sub(2), self.prompt_cursor(draft));
        let start = input.window_start(content_h);
        let mut text_rows: Vec<_> = input.rows.iter().skip(start).take(content_h)
            .enumerate().map(|(index, text)| format!("{}{text}", if index + start == 0 { "> " } else { "  " })).collect();
        while text_rows.len() < content_h {
            text_rows.push(String::new());
        }
        let Some(border) = crate::native_box_style::TextFrame::new(frame, width, pad) else {
            return self.render_typed_classified(width, height, workspace, messages, draft, busy);
        };
        rows.push((border.edge(false), ChatRowClass::Composer));
        for _ in 0..pad { rows.push((border.blank(), ChatRowClass::Composer)); }
        for row in text_rows { rows.push((border.row(&row), ChatRowClass::Composer)); }
        for _ in 0..pad { rows.push((border.blank(), ChatRowClass::Composer)); }
        rows.push((border.edge(true), ChatRowClass::Composer));
        rows
    }

    fn typed_window(
        &self,
        width: usize,
        height: usize,
        messages: &[ChatMessage],
    ) -> Vec<(String, ChatRowClass)> {
        if height == 0 || messages.is_empty() {
            return Vec::new();
        }
        let target = self.scroll.get().saturating_add(height);
        let mut rev: Vec<(String, ChatRowClass)> = Vec::with_capacity(target.min(4096));
        'walk: for (index, message) in messages.iter().enumerate().rev() {
            let rows = typed_message_rows(width, message, starts_new_group(index, messages));
            for row in rows.into_iter().rev() {
                rev.push(row);
                if rev.len() >= target {
                    break 'walk;
                }
            }
        }
        rev.reverse();
        let total = rev.len();
        if total == 0 {
            return Vec::new();
        }
        let effective_scroll = self.scroll.get().min(total.saturating_sub(height));
        self.scroll.set(effective_scroll);
        let end = total - effective_scroll;
        let start = end.saturating_sub(height);
        rev[start..end].to_vec()
    }

    pub fn render_styled(
        &self,
        width: usize,
        height: usize,
        workspace: &str,
        messages: &[ChatMessage],
        draft: &str,
        busy: bool,
        appearance: &crate::native_box_style::BoxAppearance,
    ) -> Vec<StyledChatRow> {
        self.render_styled_mode(width, height, workspace, messages, draft, busy, appearance, true).0
    }

    #[must_use]
    pub fn render_styled_with_spans(
        &self, width: usize, height: usize, workspace: &str, messages: &[ChatMessage],
        draft: &str, busy: bool, appearance: &crate::native_box_style::BoxAppearance,
    ) -> (Vec<StyledChatRow>, Vec<Vec<ChatRunSpan>>) {
        self.render_styled_mode(width, height, workspace, messages, draft, busy, appearance, true)
    }

    pub fn render_styled_without_prompt(
        &self, width: usize, height: usize, workspace: &str, messages: &[ChatMessage],
        busy: bool, appearance: &crate::native_box_style::BoxAppearance,
    ) -> Vec<StyledChatRow> {
        self.render_styled_mode(width, height, workspace, messages, "", busy, appearance, false).0
    }

    #[must_use]
    pub fn render_styled_without_prompt_with_spans(
        &self, width: usize, height: usize, workspace: &str, messages: &[ChatMessage],
        busy: bool, appearance: &crate::native_box_style::BoxAppearance,
    ) -> (Vec<StyledChatRow>, Vec<Vec<ChatRunSpan>>) {
        self.render_styled_mode(width, height, workspace, messages, "", busy, appearance, false)
    }

    fn render_styled_mode(
        &self, width: usize, height: usize, workspace: &str, messages: &[ChatMessage],
        draft: &str, busy: bool, appearance: &crate::native_box_style::BoxAppearance,
        show_prompt: bool,
    ) -> (Vec<StyledChatRow>, Vec<Vec<ChatRunSpan>>) {
        let requested_scroll = self.scroll.get();
        let height = height.min(MAX_VIEW_ROWS);
        let width = width.min(u16::MAX as usize);
        let draft = clip_utf8_prefix_bytes(draft, MAX_DRAFT_BYTES);
        let prompt = session_composer_layout(width, height, draft, appearance);
        let mut rows = if !show_prompt {
            let mut rows = vec![(String::new(), ChatRowClass::Blank); height];
            if let Some(header) = rows.first_mut() {
                *header = (render_header(width, workspace, busy, self.scroll.get()), ChatRowClass::Header);
            }
            rows
        } else if prompt.rail {
            let mut rows = vec![(String::new(), ChatRowClass::Blank); height - prompt.height];
            if let Some(header) = rows.first_mut() {
                *header = (render_header(width, workspace, busy, self.scroll.get()), ChatRowClass::Header);
            }
            let blank = format!("┃{}", " ".repeat(width - 1));
            rows.push((blank.clone(), ChatRowClass::Composer));
            let inset = if width < 40 { 1 } else { 2 };
            let text_width = width.saturating_sub(1 + 2 * inset);
            let input = crate::native_prompt_layout::wrap_prompt(draft, text_width, self.prompt_cursor(draft));
            let start = input.window_start(prompt.height - 4);
            for line in input.rows.iter().skip(start).take(prompt.height - 4) {
                rows.push((format!("┃{}{}", " ".repeat(inset),
                    crate::native_view_layout::slice_cells(&line.replace('\t', " "), 0, text_width as u16)), ChatRowClass::Composer));
            }
            rows.push((blank.clone(), ChatRowClass::Composer));
            rows.push((blank, ChatRowClass::Composer));
            rows.push((format!("╹{}", "▀".repeat(width - 1)), ChatRowClass::Composer));
            rows
        } else if !prompt.framed {
            let mut rows = vec![(String::new(), ChatRowClass::Blank); height.saturating_sub(prompt.height)];
            if let Some(header) = rows.first_mut() {
                *header = (render_header(width, workspace, busy, self.scroll.get()), ChatRowClass::Header);
            }
            let text_width = width.saturating_sub(2);
            let input = crate::native_prompt_layout::wrap_prompt(draft, text_width, self.prompt_cursor(draft));
            let start = input.window_start(prompt.height);
            for (index, line) in input.rows.iter().skip(start).take(prompt.height).enumerate() {
                let prefix = if index + start == 0 { "> " } else { "  " };
                rows.push((truncate_row(&format!("{prefix}{line}"), width), ChatRowClass::Composer));
            }
            rows
        } else { self.render_typed_classified_framed(
            width,
            height,
            workspace,
            // Establish the frame using the same message set as the styled
            // transcript.  This keeps the body/composer split identical while
            // the boxed-row pass below remains the source of truth for scroll
            // bounds and visible content.
            &[],
            draft,
            busy,
            appearance.effective(crate::native_box_style::BoxId::ChatComposer),
        ) };
        // The first pass is only the shared frame/composer layout. Restore
        // the caller's scroll request before calculating the actual boxed
        // transcript window; typed and boxed rows can have different heights.
        self.scroll.set(requested_scroll);
        let body_h = rows
            .iter()
            .position(|(_, class)| *class == ChatRowClass::Composer)
            .unwrap_or(rows.len());
        let transcript_h = body_h.saturating_sub(1);
        let target = self.scroll.get().saturating_add(transcript_h);
        let mut reversed: Vec<(StyledChatRow, Vec<ChatRunSpan>)> = Vec::new();
        if transcript_h > 0 {
            'messages: for (index, message) in messages.iter().enumerate().rev() {
                let mut message_spans: Vec<(usize, Vec<ChatRunSpan>)> = Vec::new();
                let message_rows = boxed_message_rows_spans(
                    width,
                    message,
                    starts_styled_group(index, messages),
                    appearance,
                    &mut message_spans,
                );
                let mut dense = vec![Vec::new(); message_rows.len()];
                for (row, spans) in message_spans {
                    if row < dense.len() { dense[row] = spans; }
                }
                for (row, spans) in message_rows.into_iter().zip(dense).rev() {
                    reversed.push((row, spans));
                    if reversed.len() >= target {
                        break 'messages;
                    }
                }
            }
        }
        reversed.reverse();
        let effective_scroll = self.scroll.get().min(reversed.len().saturating_sub(transcript_h));
        // Synchronize the stored offset with the current rendered content.
        // Without this, an overscroll remains at MAX_SCROLL_ROWS and every
        // later wheel-down must pay thousands of steps before moving.
        self.scroll.set(effective_scroll);
        let end = reversed.len().saturating_sub(effective_scroll);
        let start = end.saturating_sub(transcript_h);
        let visible = reversed[start..end].to_vec();
        // OpenTUI's scrollbox follows the tail only once content overflows;
        // shorter conversations start after its single top spacer. Padding
        // belongs below the conversation, never between that spacer and User.
        let first = rows.iter().take(body_h)
            .position(|(_, class)| *class == ChatRowClass::Header)
            .map_or(0, |header| header + 1);
        let mut bounds = vec![None; rows.len()];
        let mut row_spans: Vec<Vec<ChatRunSpan>> = vec![Vec::new(); rows.len()];
        for (index, ((text, class, range), spans)) in visible.into_iter().enumerate() {
            rows[first + index] = (text, class);
            bounds[first + index] = range;
            row_spans[first + index] = spans;
        }
        let styled: Vec<StyledChatRow> = rows.into_iter()
            .zip(bounds)
            .map(|((text, class), bounds)| (text, class, bounds))
            .collect();
        (styled, row_spans)
    }

    /// Legacy callers have text rows, not role facts. Reuse the exact prompt
    /// scaffold without inventing user/assistant roles for their transcript.
    pub fn render_legacy_styled(
        &self, width: usize, height: usize, workspace: &str,
        transcript: &[String], draft: &str, busy: bool,
        appearance: &crate::native_box_style::BoxAppearance,
    ) -> Vec<StyledChatRow> {
        self.render_legacy_styled_mode(width, height, workspace, transcript, draft, busy, appearance, true)
    }

    pub fn render_legacy_without_prompt(
        &self, width: usize, height: usize, workspace: &str, transcript: &[String],
        busy: bool, appearance: &crate::native_box_style::BoxAppearance,
    ) -> Vec<StyledChatRow> {
        self.render_legacy_styled_mode(width, height, workspace, transcript, "", busy, appearance, false)
    }

    fn render_legacy_styled_mode(
        &self, width: usize, height: usize, workspace: &str, transcript: &[String],
        draft: &str, busy: bool, appearance: &crate::native_box_style::BoxAppearance,
        show_prompt: bool,
    ) -> Vec<StyledChatRow> {
        let scroll = self.scroll.get();
        let mut rows = self.render_styled_mode(width, height, workspace, &[], draft, busy, appearance, show_prompt).0;
        self.scroll.set(scroll);
        let body = rows.iter().position(|(_, class, _)| *class == ChatRowClass::Composer).unwrap_or(rows.len());
        let first = usize::from(body > 0);
        let window = self.transcript_window(width, body.saturating_sub(first), transcript);
        for (index, text) in window.into_iter().enumerate() {
            rows[first + index] = (text, ChatRowClass::Text, Some((0, width.min(u16::MAX as usize) as u16)));
        }
        rows
    }

    pub fn styled_message_rows(
        &self,
        width: usize,
        messages: &[ChatMessage],
        appearance: &crate::native_box_style::BoxAppearance,
    ) -> usize {
        messages.iter().enumerate().map(|(index, message)|
            boxed_message_rows(width, message, starts_styled_group(index, messages), appearance).len()
        ).sum()
    }

    /// Anchor the last currently visible transcript row to a message and an
    /// intra-message rendered-row offset. The caller supplies the same full
    /// projection that is painted, preserving group-boundary semantics.
    pub fn rendered_anchor(
        &self,
        width: usize,
        messages: &[ChatMessage],
        ids: &[String],
        appearance: &crate::native_box_style::BoxAppearance,
    ) -> Option<RenderedChatAnchor> {
        let mut total = 0usize;
        let mut spans = Vec::with_capacity(messages.len());
        for (index, message) in messages.iter().enumerate() {
            let rows = boxed_message_rows(width, message, starts_styled_group(index, messages), appearance).len();
            spans.push((total, rows));
            total = total.saturating_add(rows);
        }
        let target = total.checked_sub(self.scroll.get().saturating_add(1))?;
        spans.iter().enumerate().find_map(|(index, (start, rows))| {
            (target >= *start && target < start.saturating_add(*rows))
                .then_some(RenderedChatAnchor {
                    id: ids.get(index)?.clone(), index, row: target - *start,
                })
        })
    }

    /// Return the tail-relative scroll needed to keep an anchor at the same
    /// screen row after the complete rendered projection changes.
    pub fn scroll_for_anchor(
        &self,
        width: usize,
        messages: &[ChatMessage],
        ids: &[String],
        appearance: &crate::native_box_style::BoxAppearance,
        anchor: &RenderedChatAnchor,
        screen_row_from_bottom: usize,
    ) -> Option<usize> {
        let mut total = 0usize;
        let mut anchor_start = None;
        for (index, message) in messages.iter().enumerate() {
            let rows = boxed_message_rows(width, message, starts_styled_group(index, messages), appearance).len();
            if ids.get(index).map(String::as_str) == Some(anchor.id.as_str()) {
                anchor_start = Some((total, rows));
            }
            total = total.saturating_add(rows);
        }
        let (start, rows) = anchor_start?;
        if anchor.row >= rows { return None; }
        // The captured row was the bottom visible transcript row. Locate the
        // same identity in the new projection, then derive its exact
        // tail-relative offset. This handles eviction changing indices and
        // separators changing the preceding row count.
        let absolute = start + anchor.row;
        Some(total.saturating_sub(absolute.saturating_add(1)).saturating_add(screen_row_from_bottom))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_facts_map_onto_rendered_rows_without_painted_inference() {
        let text = "before\n\n**bold** word\n~~~\ncode\n~~~\nafter";
        let mut rows = Vec::new();
        push_markdown_wrapped(&mut rows, 40, text, "", ChatRowClass::AssistantCard);
        let spans = markdown_part_row_spans(text, 40, "", &rows);
        assert_eq!(rows.len(), 7);
        assert_eq!(spans.len(), rows.len());
        assert!(spans[0].is_empty());
        assert!(spans[1].is_empty());
        assert_eq!(spans[2], vec![ChatRunSpan { start: 2, end: 6, flags: crate::markdown_runs::BOLD }]);
        for (row, end) in [(3, 3), (4, 4), (5, 3)] {
            assert_eq!(spans[row], vec![ChatRunSpan { start: 0, end, flags: crate::markdown_runs::CODE | crate::markdown_runs::CODE_BLOCK }]);
        }
        assert!(spans[6].is_empty());
    }

    #[test]
    fn markdown_code_spans_distinguish_inline_background_from_block_background() {
        let inline = "before `code` after";
        let mut rows = Vec::new();
        push_markdown_wrapped(&mut rows, 40, inline, "", ChatRowClass::AssistantCard);
        let spans = markdown_part_row_spans(inline, 40, "", &rows);
        let code = spans[0].iter().find(|span| span.flags & crate::markdown_runs::CODE != 0).expect("inline code span");
        assert_eq!(code.flags & crate::markdown_runs::CODE_BLOCK, 0);
        let block = "~~~\ncode\n~~~";
        rows.clear();
        push_markdown_wrapped(&mut rows, 40, block, "", ChatRowClass::AssistantCard);
        let spans = markdown_part_row_spans(block, 40, "", &rows);
        assert_eq!(spans.len(), 3);
        for row in spans { assert_ne!(row.first().expect("block span").flags & crate::markdown_runs::CODE_BLOCK, 0); }
    }

    #[test]
    fn markdown_facts_reset_fence_per_part_and_keep_links_inert() {
        let text = "~~~\ncode\n~~~\n**a**";
        let mut rows = Vec::new();
        push_markdown_wrapped(&mut rows, 40, text, "", ChatRowClass::AssistantCard);
        let spans = markdown_part_row_spans(text, 40, "", &rows);
        assert_eq!(rows.len(), 4);
        assert!(!spans[0].is_empty());
        assert!(spans[0].iter().all(|span| span.flags & crate::markdown_runs::CODE != 0 && span.flags & crate::markdown_runs::CODE_BLOCK != 0));
        assert_eq!(spans[3], vec![ChatRunSpan { start: 2, end: 3, flags: crate::markdown_runs::BOLD }]);
        rows.clear();
        push_markdown_wrapped(&mut rows, 40, "**b**", "", ChatRowClass::AssistantCard);
        assert_eq!(markdown_part_row_spans("**b**", 40, "", &rows)[0], vec![ChatRunSpan { start: 2, end: 3, flags: crate::markdown_runs::BOLD }]);
        let linked = "see [x](https://example.test)";
        rows.clear();
        push_markdown_wrapped(&mut rows, 40, linked, "  ", ChatRowClass::ToolResult);
        let spans = markdown_part_row_spans(linked, 40, "  ", &rows);
        let link = spans[0].iter().find(|span| span.flags & crate::markdown_runs::LINK != 0).expect("inert link span");
        assert_eq!((link.start, link.end), (7, 8));
    }

    #[test]
    fn markdown_fenced_code_preserves_whitespace_and_unicode_when_wrapped() {
        let source = "  let label = \"界e\u{301}\";  \n  println!(\"{label}\");";
        let input = format!("intro prose wraps across this narrow pane\n~~~rust\n{source}\n~~~\noutro prose wraps too");
        let message = ChatMessage::text(ChatRole::Assistant, input);
        let rows = typed_message_rows(12, &message, false);

        let opening = rows.iter().position(|(row, _)| row == "~~~rust").unwrap();
        let closing = rows.iter().position(|(row, _)| row == "~~~").unwrap();
        let code_rows: Vec<&str> = rows[opening + 1..closing]
            .iter()
            .map(|(row, _)| row.as_str())
            .collect();
        // A rendered row is a physical display row: source newlines are
        // boundaries between rows, not characters stored in a row. Rebuild
        // each physical source line independently so the assertion checks
        // content, indentation, trailing spaces, and Unicode without
        // manufacturing a newline that the row model does not represent.
        let boundary = code_rows
            .iter()
            .position(|row| row.ends_with("  "))
            .expect("first fenced source line keeps its trailing spaces");
        let first_line: String = code_rows[..=boundary].concat();
        let second_line: String = code_rows[boundary + 1..].concat();
        let mut source_lines = source.split('\n');
        assert_eq!(first_line, source_lines.next().unwrap());
        assert_eq!(second_line, source_lines.next().unwrap());
        assert!(source_lines.next().is_none());
        assert!(rows.iter().any(|(row, _)| row == "intro prose"));
        assert!(rows.iter().all(|(row, _)| row.chars().count() <= 12));
    }

    #[test]
    fn markdown_fence_marker_and_minimum_run_control_closing() {
        let input = "~~~~text\n  first  \n```\nsecond\n~~~~\nafter";
        let message = ChatMessage::text(ChatRole::Assistant, input);
        let rows = typed_message_rows(8, &message, false);
        let opening = rows.iter().position(|(row, _)| row == "~~~~text").unwrap();
        let closing = rows.iter().position(|(row, _)| row == "~~~~").unwrap();
        assert_eq!(
            rows[opening + 1..opening + 3]
                .iter()
                .map(|(row, _)| row.as_str())
                .collect::<String>(),
            "  first  ",
        );
        assert_eq!(rows[opening + 3].0, "```");
        assert_eq!(rows[opening + 4].0, "second");
        assert_eq!(rows[closing + 1].0, "after");
        assert!(rows.iter().all(|(row, _)| row.chars().count() <= 8));
    }

    #[test]
    fn markdown_code_wrap_handles_narrow_unicode_without_losing_ascii() {
        let input = "```\n界a界b\n```";
        let message = ChatMessage::text(ChatRole::Assistant, input);
        let rows = typed_message_rows(1, &message, false);
        assert!(rows.iter().all(|(row, _)| row.chars().count() <= 1));
        let rendered: String = rows.iter().map(|(row, _)| row.as_str()).collect();
        assert!(rendered.contains("u{754c}"), "narrow pane keeps the wide codepoint as an explicit escape: {rendered:?}");
        assert!(rows.iter().any(|(row, _)| row == "a"));
        assert!(rows.iter().any(|(row, _)| row == "b"));
    }

    #[test]
    fn markdown_code_preserves_long_combining_and_join_sequences_visibly() {
        let combining = format!("a{}", "\u{301}".repeat(12));
        let joined = "👩‍👩‍👩‍👩‍👩";
        let byte_heavy = format!(
            "a{}‍b{}‍c{}‍d",
            "\u{301}".repeat(8),
            "\u{301}".repeat(8),
            "\u{301}".repeat(8),
        );
        for (source, marker, expected) in [
            (combining.as_str(), "\\u{301}", 12),
            (joined, "\\u{200d}", 4),
            (byte_heavy.as_str(), "\\u{301}", 24),
            ("🇦", "\\u{1f1e6}", 1),
        ] {
            let message = ChatMessage::text(ChatRole::Assistant, format!("```\n{source}\n```"));
            let rows = typed_message_rows(40, &message, false);
            let rendered: String = rows.iter().map(|(row, _)| row.as_str()).collect();
            assert_eq!(rendered.matches(marker).count(), expected, "{rendered:?}");
        }
    }

    #[test]
    fn markdown_code_preserves_tabs_and_unrelated_unicode_after_bad_cluster() {
        let source = "\ta\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}界";
        let message = ChatMessage::text(ChatRole::Assistant, format!("```\n{source}\n```"));
        let rows = typed_message_rows(80, &message, false);
        let rendered: String = rows.iter().map(|(row, _)| row.as_str()).collect();
        assert!(!rendered.contains('\t'), "fenced source tabs expand to paint-safe spaces: {rendered:?}");
        assert!(rendered.contains('界'), "unrelated valid scalar must not be line-globally escaped: {rendered:?}");
        assert!(!rendered.contains("\\u{754c}"), "following 界 must not be escaped: {rendered:?}");
    }

    #[test]
    fn markdown_code_tabs_use_shared_cluster_cells_at_eight_column_stops() {
        assert_eq!(expand_code_tabs("é\tX"), "é       X");
        assert_eq!(expand_code_tabs("👩‍💻\tX"), "👩‍💻      X");
        let rows = typed_message_rows(
            80,
            &ChatMessage::text(ChatRole::Assistant, "```\n👩‍💻\tX\n```"),
            false,
        );
        let code = rows.iter().find(|(row, _)| row.contains('X')).unwrap();
        assert_eq!(code.0, "👩‍💻      X");
    }

    #[test]
    fn markdown_code_escapes_standalone_regional_indicator_locally() {
        for suffix in ["ASCII", "界"] {
            let message = ChatMessage::text(
                ChatRole::Assistant,
                format!("```\n🇦{suffix}\n```"),
            );
            let rows = typed_message_rows(80, &message, false);
            let rendered: String = rows.iter().map(|(row, _)| row.as_str()).collect();
            assert!(rendered.contains("\\u{1f1e6}"), "{rendered:?}");
            assert!(rendered.contains(suffix), "unrelated suffix was lost: {rendered:?}");
        }
    }

    #[test]
    fn markdown_code_strips_escape_state_across_physical_lines() {
        let input = "```\n\u{1b}]8;;https://example.invalid\nVISIBLE\u{1b}\\\nSAFE\n```";
        let rows = typed_message_rows(80, &ChatMessage::text(ChatRole::Assistant, input), false);
        let rendered: String = rows.iter().map(|(row, _)| row.as_str()).collect();
        assert!(!rendered.contains("VISIBLE"), "OSC payload must remain hidden: {rendered:?}");
        assert!(rendered.contains("SAFE"), "post-terminator line remains visible: {rendered:?}");
        assert!(!rendered.contains("https://example.invalid"), "OSC payload leaked: {rendered:?}");
    }

    #[test]
    fn markdown_code_marks_input_and_row_overflow_visibly_within_row_cap() {
        let input = format!("```\n{}", "x".repeat(MAX_INPUT_ROW_BYTES));
        let rows = typed_message_rows(0, &ChatMessage::text(ChatRole::Assistant, input), false);
        assert!(rows.iter().any(|(row, _)| row.contains("[display truncated]")));
        assert!(rows.len() <= MAX_MESSAGE_ROWS);

        let lines = vec!["x"; MAX_MESSAGE_ROWS + 8].join("\n");
        let message = ChatMessage::text(ChatRole::Assistant, format!("```\n{lines}\n```"));
        let rows = typed_message_rows(40, &message, false);
        assert_eq!(rows.len(), MAX_MESSAGE_ROWS);
        assert!(rows.last().unwrap().0.contains("! content omitted"));
    }

    #[test]
    fn markdown_non_ascii_fence_suffix_does_not_close_code() {
        let message = ChatMessage::text(
            ChatRole::Assistant,
            "```\nbody\n```\u{00a0}\nstill fenced\n```",
        );
        let rows = typed_message_rows(80, &message, false);
        let suffix = rows.iter().position(|(row, _)| row == "```\u{00a0}").unwrap();
        let still_fenced = rows.iter().position(|(row, _)| row == "still fenced").unwrap();
        assert!(still_fenced > suffix);
        assert_eq!(rows.last().unwrap().0, "```");
    }

    #[test]
    fn unicode_cell_width_truncate_row_respects_cells_and_tail() {
        let fitting = "界e\u{0301}🦀";
        assert_eq!(truncate_row(fitting, 5), fitting);
        assert_eq!(truncate_row("界界界tail", 6), "界界界");
    }

    #[test]
    fn styled_message_boxes_keep_real_text_bounds_separate_from_decoration() {
        use crate::native_box_style::{BoxAppearance, RadiusPreset};
        let mut style = BoxAppearance::default();
        style.preset = RadiusPreset::IosSquircle;
        style.padding = 1;
        // Squircle mode explicitly requests a closed outline for every
        // bordered message type, regardless of the upstream rail default.
        for role in [ChatRole::User, ChatRole::Assistant, ChatRole::System, ChatRole::Tool] {
            let message = ChatMessage::text(role, "ACTUAL_TEXT");
            let rows = boxed_message_rows(40, &message, false, &style);
            assert_eq!(rows.first().unwrap().0, format!("╭{}╮", "─".repeat(38)));
            assert_eq!(rows.last().unwrap().0, format!("╰{}╯", "─".repeat(38)));
            assert!(rows.iter().all(|(line, _, _)|
                crate::native_view_layout::fit(line, 40) == *line),
                "every painted squircle row must fit its box: {rows:?}");
            assert_eq!(rows[0].2, None, "decoration rows carry no text bounds");
            let (text, _, bounds) = rows
                .iter()
                .find(|(text, _, _)| text.contains("ACTUAL_TEXT"))
                .unwrap();
            let (left, width) = bounds.unwrap();
            let content = crate::native_view_layout::slice_cells(text, left, width);
            assert_eq!(content.trim(), "ACTUAL_TEXT");
            assert!(!content.contains('│'));
        }
    }

    /// Radius is decoupled from chat framing: for every role, every logical
    /// radius renders IDENTICAL message rows (same margin/padding at 0 and
    /// 50, no rails added, no top/bottom borders appear), while the
    /// composer — a surface that already paints corners — keeps changing
    /// exactly its corner glyphs.
    #[test]
    fn radius_change_never_frames_messages_or_alters_padding() {
        use crate::native_box_style::{BoxAppearance, RadiusPreset};
        let messages = vec![
            ChatMessage::text(ChatRole::User, "user note"),
            ChatMessage::text(ChatRole::Assistant, "assistant prose"),
            ChatMessage {
                role: ChatRole::Assistant,
                parts: vec![ChatPart::ToolCall {
                    name: "bash".to_owned(),
                    id: "call-1".to_owned(),
                }],
                agent: None,
                model: None,
                assistant_facts: None,
            },
        ];
        let appearance = |radius: u8| {
            let mut style = BoxAppearance::default();
            style.preset = RadiusPreset::Custom;
            style.radius = radius;
            style
        };
        for radius in [0u8, 1, 17, 50] {
            for message in &messages {
                let rows = boxed_message_rows(60, message, false, &appearance(radius));
                assert!(
                    !rows
                        .iter()
                        .any(|(text, _, _)| text.contains('╔')
                            || text.contains('╭')
                            || text.contains('┏')),
                    "radius {radius} must not create frame rows: {rows:?}"
                );
                assert_eq!(
                    rows,
                    boxed_message_rows(60, message, false, &appearance(0)),
                    "message rows identical at radius {radius} and 0"
                );
            }
        }
        // Styled path: transcript rows identical at radius 0/50 (padding
        // unchanged), composer corners DO change (existing surface).
        let mut view = ChatView::new();
        for radius in [0u8, 50] {
            let rows = view.render_styled(
                60,
                20,
                "ws",
                &messages,
                "draft",
                false,
                &appearance(radius),
            );
            let body: Vec<_> = rows
                .split(|(_, class, _)| *class == ChatRowClass::Composer)
                .next()
                .unwrap()
                .to_vec();
            if radius == 0 {
                assert_eq!(
                    body,
                    view.render_styled(60, 20, "ws", &messages, "draft", false, &appearance(50))
                        .split(|(_, class, _)| *class == ChatRowClass::Composer)
                        .next()
                        .unwrap()
                        .to_vec(),
                    "padding/margins identical at radius 0 and 50"
                );
            }
        }
        // Radius changes the EXISTING composer corners only.
        let composer_top = |radius: u8| {
            view.render_styled(60, 20, "ws", &messages, "draft", false, &appearance(radius))
                .iter()
                .find(|(_, class, _)| *class == ChatRowClass::Composer)
                .map(|(text, _, _)| text.as_str())
                .unwrap_or_default()
                .to_owned()
        };
        assert!(composer_top(0).starts_with('┌'));
        assert!(composer_top(50).starts_with('╭'));
    }

    /// Default user card = upstream left rail + inset (distinct treatment,
    /// never a full rectangle): rail rows start with the rail glyph, no
    /// message row carries a right rail or a bottom/top full border, and
    /// assistant prose stays unbordered beside it.
    #[test]
    fn user_card_is_a_left_rail_and_never_a_full_frame() {
        use crate::native_box_style::BoxAppearance;
        let mut view = ChatView::new();
        let messages = vec![
            ChatMessage::text(ChatRole::User, "hello rail"),
            ChatMessage::text(ChatRole::Assistant, "prose body"),
        ];
        let rows = view.render_styled(
            60,
            18,
            "ws",
            &messages,
            "",
            false,
            &BoxAppearance::default(),
        );
        let painted: Vec<_> = rows
            .split(|(_, class, _)| *class == ChatRowClass::Composer)
            .next()
            .unwrap()
            .to_vec();
        let user_rows: Vec<_> = painted
            .iter()
            .filter(|(row, class, _)| {
                *class == ChatRowClass::UserCard && row.contains("hello rail")
            })
            .collect();
        assert!(!user_rows.is_empty(), "user text rendered");
        for (row, _, _) in &user_rows {
            assert!(row.starts_with(USER_RAIL), "left rail first cell: {row:?}");
            assert_ne!(
                row.chars().last(),
                Some(USER_RAIL),
                "no right rail: {row:?}"
            );
            for (ch_l, ch_r) in row.chars().zip(row.chars().skip(1)) {
                assert!(
                    !matches!((ch_l, ch_r), ('┌', '─') | ('╭', '─') | ('╔', '═') | ('┏', '━')),
                    "no top border run: {row:?}"
                );
            }
        }
        let prose: Vec<_> = painted
            .iter()
            .filter(|(row, class, _)| {
                *class == ChatRowClass::AssistantCard && row.contains("prose body")
            })
            .collect();
        assert!(!prose.is_empty(), "assistant prose rendered");
        for (row, _, _) in &prose {
            assert!(
                !row.chars().any(|c| matches!(c, '╭' | '┌' | '╔' | '┏' | '┃')),
                "assistant prose stays unbordered: {row:?}"
            );
        }
    }

    /// Selected OpenCode's UserMessage has one rail cell, two columns of
    /// content inset, and one raised padding row on either side. Its sibling
    /// assistant TextPart has a three-column inset and no card-padding rows.
    #[test]
    fn pinned_message_role_geometry_and_literal_user_text() {
        let appearance = crate::native_box_style::BoxAppearance::default();
        for width in [12, 116, 265] {
            let user = boxed_message_rows(
                width,
                &ChatMessage::text(ChatRole::User, "hello"),
                false,
                &appearance,
            );
            assert_eq!(user.len(), 3, "one padding row on each side");
            assert_eq!(user[1].0.trim_end(), "┃  hello");
            assert_eq!(user[1].2, Some((3, (width - 3) as u16)));
            for padding in [&user[0], &user[2]] {
                assert_eq!(padding.1, ChatRowClass::UserCard, "raised fill continues across padding");
                assert_eq!(padding.2, None, "card padding is not selectable text");
            }
            let assistant = boxed_message_rows(
                width,
                &ChatMessage::text(ChatRole::Assistant, "hello"),
                false,
                &appearance,
            );
            assert_eq!(assistant.len(), 1, "assistant text has no card padding");
            assert_eq!(assistant[0].0.trim_end(), "   hello");
            assert_eq!(assistant[0].1, ChatRowClass::AssistantCard);
            assert_eq!(assistant[0].2, Some((3, (width - 3) as u16)));
        }
        let literal = boxed_message_rows(
            40,
            &ChatMessage::text(ChatRole::User, "**literal** `code`"),
            false,
            &appearance,
        );
        assert_eq!(literal[1].0.trim_end(), "┃  **literal** `code`");
    }

    #[test]
    fn pinned_short_conversation_starts_at_top_with_assistant_row_gap() {
        let view = ChatView::new();
        let messages = [
            ChatMessage::text(ChatRole::User, "question"),
            ChatMessage::text(ChatRole::Assistant, "answer"),
        ];
        for height in [18, 40, 59] {
            let rows = view.render_styled(60, height, "workspace", &messages, "", false,
                &crate::native_box_style::BoxAppearance::default());
            assert_eq!(rows[0].1, ChatRowClass::Header, "the header/spacer is not overwritten");
            assert!(rows[0].0.contains("workspace"), "helper header content is preserved");
            assert_eq!(rows[1].1, ChatRowClass::UserCard, "user padding starts after the scrollbox spacer");
            assert_eq!(rows[2].0.trim_end(), "┃  question");
            assert_eq!(rows[3].1, ChatRowClass::UserCard);
            assert_eq!(rows[4].1, ChatRowClass::Separator, "assistant row wrapper marginTop=1");
            assert_eq!(rows[5].0.trim_end(), "   answer");
            assert!(rows.iter().skip(6).take_while(|(_, class, _)| *class != ChatRowClass::Composer)
                .all(|(text, _, _)| text.trim().is_empty()), "unused space follows the conversation");
        }
    }

    /// Existing explicit heavy/double border preferences remain full frames
    /// (never silently downgraded by the radius decoupling).
    #[test]
    fn explicit_heavy_and_double_line_weights_still_frame_messages() {
        use crate::native_box_style::{
            BorderStyle, BoxAppearance, BoxId, BoxOverride,
        };
        let mut style = BoxAppearance::default();
        style.overrides.insert(
            BoxId::User,
            BoxOverride {
                border_style: Some(BorderStyle::Double),
                ..BoxOverride::default()
            },
        );
        let mut heavy = BoxAppearance::default();
        heavy.border_style = BorderStyle::Heavy;
        for (name, appearance) in [("double", &style), ("heavy", &heavy)] {
            let rows = boxed_message_rows(
                40,
                &ChatMessage::text(ChatRole::User, "kept"),
                false,
                appearance,
            );
            assert!(
                rows.iter()
                    .any(|(text, _, _)| text.starts_with(['╔', '┏'])),
                "{name}: full frame kept: {rows:?}"
            );
            assert!(
                rows.iter()
                    .any(|(text, _, _)| text.trim_end().ends_with(['╗', '┛'])),
                "{name}: right rail kept: {rows:?}"
            );
        }
        // radius edits on top of these styles do not change the frame.
        let mut rounded = style.clone();
        rounded.preset = crate::native_box_style::RadiusPreset::Rounded;
        assert_eq!(
            boxed_message_rows(40, &ChatMessage::text(ChatRole::User, "kept"), false, &style)
                .iter()
                .map(|(t, _, _)| t.clone())
                .collect::<Vec<_>>(),
            boxed_message_rows(40, &ChatMessage::text(ChatRole::User, "kept"), false, &rounded)
                .iter()
                .map(|(t, _, _)| t.clone())
                .collect::<Vec<_>>(),
        );
    }

    fn history(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("entry {i}")).collect()
    }

    fn history_message_id(id: usize) -> String {
        format!("00000000-0000-4000-8000-{id:012}")
    }

    fn retained_history_user_record(id: usize) -> serde_json::Value {
        serde_json::json!({
            "id": history_message_id(id),
            "role": "user",
            "body": {"storage": "inline", "text": format!("shared-row-{id:03}")}
        })
    }

    fn projected_anchor_record(id: usize) -> serde_json::Value {
        let message_id = format!("msg-{id:03}");
        match id % 5 {
            0 => {
                let text = if id == 115 {
                    format!(
                        "opening prose for message {message_id} deliberately wraps across a narrow user card\nanchor-row-{message_id}\nclosing prose for {message_id} anchor-115"
                    )
                } else {
                    format!("user message {message_id} with a multiline card\nsecond line")
                };
                serde_json::json!({
                    "id": message_id,
                    "role": "user",
                    "body": {"storage": "inline", "text": text}
                })
            }
            1 | 4 => serde_json::json!({
                "id": message_id,
                "role": "assistant",
                "body": {"storage": "inline", "text": format!("assistant message {message_id}")}
            }),
            2 => serde_json::json!({
                "id": message_id,
                "role": "assistant",
                "parts": [
                    {"type": "text", "text": format!("tool request from {message_id}")},
                    {"type": "tool_call", "name": "bash", "call_id": format!("call-{id}"), "arguments": {"cmd": "true"}}
                ],
                "tool_output": {
                    "name": "bash",
                    "call_id": format!("call-{id}"),
                    "output": format!("tool output {id}"),
                    "is_error": false
                }
            }),
            _ => serde_json::json!({
                "id": message_id,
                "role": "tool",
                "body": {"storage": "inline", "text": format!("tool message {message_id}")}
            }),
        }
    }

    fn projected_anchor_history(
        start: usize,
        end: usize,
    ) -> Vec<(String, ChatMessage)> {
        let records: Vec<_> = (start..end).map(projected_anchor_record).collect();
        crate::native_chat_projection::project_history_entries(&records)
    }

    fn anchor_for_marker(
        entries: &[(String, ChatMessage)],
        id: &str,
        marker: &str,
        width: usize,
        appearance: &crate::native_box_style::BoxAppearance,
    ) -> RenderedChatAnchor {
        let messages: Vec<_> = entries.iter().map(|(_, message)| message.clone()).collect();
        let (index, (entry_id, message)) = entries
            .iter()
            .enumerate()
            .find(|(_, (entry_id, _))| entry_id.as_str() == id)
            .expect("anchor message is in projected history");
        let rows = boxed_message_rows(
            width,
            message,
            starts_new_group(index, &messages),
            appearance,
        );
        assert!(
            index > 0 && starts_new_group(index, &messages),
            "anchor is at an actual group boundary"
        );
        assert_eq!(rows.first().map(|(_, class, _)| *class), Some(ChatRowClass::Separator));
        assert!(
            rows.iter()
                .filter(|(_, class, _)| *class == ChatRowClass::UserCard)
                .count()
                >= 3,
            "anchor user card is multiline and wrapped: {rows:?}"
        );
        let row = rows
            .iter()
            .position(|(text, class, _)| *class == ChatRowClass::UserCard && text.contains(marker))
            .expect("anchor marker has a rendered user-card row");
        RenderedChatAnchor {
            id: entry_id.clone(),
            index,
            row,
        }
    }

    fn place_at_anchor(
        view: &mut ChatView,
        entries: &[(String, ChatMessage)],
        width: usize,
        appearance: &crate::native_box_style::BoxAppearance,
        anchor: &RenderedChatAnchor,
    ) -> usize {
        let ids: Vec<_> = entries.iter().map(|(id, _)| id.clone()).collect();
        let messages: Vec<_> = entries.iter().map(|(_, message)| message.clone()).collect();
        let scroll = view
            .scroll_for_anchor(width, &messages, &ids, appearance, anchor, 0)
            .expect("anchor identity and rendered row exist");
        view.set_scroll(scroll);
        let captured = view
            .rendered_anchor(width, &messages, &ids, appearance)
            .expect("positioned view captures a rendered anchor");
        assert_eq!(captured.id, anchor.id);
        assert_eq!(captured.row, anchor.row);
        scroll
    }

    fn visible_rows_at_anchor(
        view: &mut ChatView,
        entries: &[(String, ChatMessage)],
        width: usize,
        height: usize,
        appearance: &crate::native_box_style::BoxAppearance,
        anchor: &RenderedChatAnchor,
        marker: &str,
    ) -> Vec<(String, ChatRowClass)> {
        let ids: Vec<_> = entries.iter().map(|(id, _)| id.clone()).collect();
        let messages: Vec<_> = entries.iter().map(|(_, message)| message.clone()).collect();
        let captured = view
            .rendered_anchor(width, &messages, &ids, appearance)
            .expect("visible view has a rendered anchor");
        assert_eq!(captured.id, anchor.id);
        assert_eq!(captured.row, anchor.row);

        let frame = view.render_styled(width, height, "anchor-test", &messages, "", false, appearance);
        assert!(frame.iter().all(|(row, _, _)| row.chars().count() <= width));
        let composer = frame
            .iter()
            .position(|(_, class, _)| *class == ChatRowClass::Composer)
            .expect("styled frame contains composer");
        let visible: Vec<_> = frame[..composer]
            .iter()
            .filter(|(_, class, _)| *class != ChatRowClass::Header)
            .map(|(row, class, _)| (row.clone(), *class))
            .collect();
        let bottom = visible.last().expect("transcript viewport is populated");
        assert_eq!(bottom.1, ChatRowClass::UserCard);
        assert!(bottom.0.contains(marker), "bottom transcript row: {bottom:?}");
        let after_render = view
            .rendered_anchor(width, &messages, &ids, appearance)
            .expect("render leaves the same anchor visible");
        assert_eq!(after_render.id, anchor.id);
        assert_eq!(after_render.row, anchor.row);
        visible
    }

    fn assert_boxed_projection_uses_parts_width_and_appearance(
        entries: &[(String, ChatMessage)],
        anchor_id: &str,
        width: usize,
        appearance: &crate::native_box_style::BoxAppearance,
    ) {
        let messages: Vec<_> = entries.iter().map(|(_, message)| message.clone()).collect();
        let mut has_tool_call = false;
        let mut has_tool_result = false;
        for (index, message) in messages.iter().enumerate() {
            let rows = boxed_message_rows(
                width,
                message,
                starts_new_group(index, &messages),
                appearance,
            );
            assert!(rows.iter().all(|(row, _, _)| row.chars().count() <= width));
            has_tool_call |= rows.iter().any(|(_, class, _)| *class == ChatRowClass::ToolCall);
            has_tool_result |= rows.iter().any(|(_, class, _)| *class == ChatRowClass::ToolResult);
        }
        assert!(has_tool_call, "projected tool-call parts render as tool cards");
        assert!(has_tool_result, "projected tool-output parts render as tool cards");

        let index = entries.iter().position(|(id, _)| id == anchor_id).unwrap();
        let user = boxed_message_rows(
            width,
            &messages[index],
            starts_new_group(index, &messages),
            appearance,
        );
        let default = boxed_message_rows(
            width,
            &messages[index],
            starts_new_group(index, &messages),
            &crate::native_box_style::BoxAppearance::default(),
        );
        assert!(user.len() > default.len(), "per-card appearance padding affects rows");
        let narrower = boxed_message_rows(
            width.saturating_sub(8),
            &messages[index],
            starts_new_group(index, &messages),
            appearance,
        );
        assert!(
            narrower.iter().filter(|(_, class, _)| *class == ChatRowClass::UserCard).count()
                > user.iter().filter(|(_, class, _)| *class == ChatRowClass::UserCard).count(),
            "narrower width wraps the actual user-card projection"
        );
    }

    #[test]
    fn rendered_anchor_survives_prepend_and_newer_history_eviction() {
        use crate::native_box_style::{BoxAppearance, BoxId, BoxOverride};

        let width = 30;
        let height = 16;
        let mut appearance = BoxAppearance::default();
        appearance.overrides.insert(
            BoxId::User,
            BoxOverride { padding: Some(2), ..BoxOverride::default() },
        );
        appearance.overrides.insert(
            BoxId::Tool,
            BoxOverride { padding: Some(1), ..BoxOverride::default() },
        );
        let before = projected_anchor_history(100, 140);
        let marker = "anchor-115";
        let seed = anchor_for_marker(&before, "msg-115", marker, width, &appearance);
        assert_boxed_projection_uses_parts_width_and_appearance(
            &before,
            "msg-115",
            width,
            &appearance,
        );

        let mut view = ChatView::new();
        let before_scroll = place_at_anchor(&mut view, &before, width, &appearance, &seed);
        let captured = view
            .rendered_anchor(
                width,
                &before.iter().map(|(_, message)| message.clone()).collect::<Vec<_>>(),
                &before.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
                &appearance,
            )
            .expect("before-page anchor captured");
        let before_visible = visible_rows_at_anchor(
            &mut view, &before, width, height, &appearance, &captured, marker,
        );

        let after = projected_anchor_history(90, 130);
        let after_index = after.iter().position(|(id, _)| id == "msg-115").unwrap();
        assert_eq!(after_index, seed.index + 10, "older page was prepended");
        let after_scroll = place_at_anchor(&mut view, &after, width, &appearance, &captured);
        assert_ne!(after_scroll, before_scroll, "newer eviction moves tail-relative window");
        let after_visible = visible_rows_at_anchor(
            &mut view, &after, width, height, &appearance, &captured, marker,
        );
        assert_eq!(before_visible, after_visible, "same rendered transcript stays anchored");
        let restored = view
            .rendered_anchor(
                width,
                &after.iter().map(|(_, message)| message.clone()).collect::<Vec<_>>(),
                &after.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
                &appearance,
            )
            .expect("after-page anchor restored");
        assert_eq!(restored.id, captured.id);
        assert_eq!(restored.row, captured.row);
        assert_eq!(restored.index, after_index);
    }

    #[test]
    fn retained_boundary_cached_newer_page_restores_anchor_before_three_row_gesture() {
        use crate::native_box_style::BoxAppearance;

        let backend_history: Vec<_> = (0..=500).map(retained_history_user_record).collect();
        let initial = &backend_history[401..501];
        let mut window = crate::native_chat_projection::NativeHistoryWindow::new(
            initial,
            Some(history_message_id(400)),
        );
        for (start, end, next_before) in [
            (301, 401, 300),
            (201, 301, 200),
            (101, 201, 100),
            (1, 101, 0),
        ] {
            let page = &backend_history[start..end];
            let next_before = history_message_id(next_before);
            assert!(window.load_older(|_, _| Ok(serde_json::json!({
                "messages": page,
                "next_before": next_before,
            }))).unwrap());
        }
        assert_eq!(window.messages().len(), crate::native_chat_projection::NativeHistoryWindow::MAX_MESSAGES);
        assert_eq!(window.messages().first().unwrap()["id"], history_message_id(1));
        assert_eq!(window.messages().last().unwrap()["id"], history_message_id(200));
        assert!(window.has_adjacent_newer(), "older-page eviction retains exactly the next newer page");

        let appearance = BoxAppearance::default();
        let before = crate::native_chat_projection::project_history_entries(window.messages());
        let anchor_id = history_message_id(200);
        let before_index = before.iter().position(|(id, _)| id == &anchor_id).unwrap();
        let before_messages: Vec<_> = before.iter().map(|(_, message)| message.clone()).collect();
        let target_rows = boxed_message_rows(
            40,
            &before_messages[before_index],
            starts_new_group(before_index, &before_messages),
            &appearance,
        );
        let anchor_row = target_rows.len() - 1;

        let mut view = crate::native_view::NativeView::new(44, 38);
        view.set_session_route(true);
        view.set_box_appearance(appearance);
        let width = view.presentation().main_width.saturating_sub(4) as usize;
        assert_eq!(width, 40, "anchor uses native view's actual transcript width");
        assert!(view.chat_at_tail(), "the cached-page input branch starts at the retained tail");
        let captured = view.chat_rendered_anchor(&before).expect("actual native view captures anchor");
        assert_eq!(captured.id, anchor_id);
        assert_eq!(captured.index, before_index);
        assert_eq!(captured.row, anchor_row, "bottom padding is part of rendered message anchor");
        let before_frame = view.render_typed("", &before_messages, false, &[]);
        let shared_markers: Vec<_> = (194..=199)
            .map(|id| format!("shared-row-{id:03}"))
            .collect();
        let before_y: Vec<_> = shared_markers
            .iter()
            .map(|marker| {
                before_frame
                    .iter()
                    .position(|row| row.contains(marker))
                    .unwrap_or_else(|| panic!("shared content {marker} appears in actual native frame"))
            })
            .collect();
        assert!(
            before_y.windows(2).all(|ys| ys[1] > ys[0]),
            "shared message rows follow actual boxed cards: {before_y:?}"
        );

        // Match the production boundary sequence: capture anchor, wheel-down,
        // admit cached page, restore anchor, re-apply the gesture exactly once.
        view.scroll_down(3);
        assert!(window.advance_newer());
        assert_eq!(window.messages().len(), crate::native_chat_projection::NativeHistoryWindow::MAX_MESSAGES);
        assert_eq!(window.messages().first().unwrap()["id"], history_message_id(101));
        assert_eq!(window.messages().last().unwrap()["id"], history_message_id(300));

        let after = crate::native_chat_projection::project_history_entries(window.messages());
        let after_index = after.iter().position(|(id, _)| id == &anchor_id).unwrap();
        assert_eq!(after_index, 99, "100 oldest records evicted from retained window");
        assert!(view.restore_chat_rendered_anchor(&after, &captured));
        let restored = view.chat_rendered_anchor(&after).expect("same backend id remains visible after cache advance");
        assert_eq!(restored.id, captured.id);
        assert_eq!(restored.row, captured.row);
        assert_eq!(restored.index, after_index);
        let restored_messages: Vec<_> = after.iter().map(|(_, message)| message.clone()).collect();
        let restored_frame = view.render_typed("", &restored_messages, false, &[]);
        let restored_y: Vec<_> = shared_markers
            .iter()
            .map(|marker| {
                restored_frame
                    .iter()
                    .position(|row| row.contains(marker))
                    .unwrap_or_else(|| panic!("shared content {marker} remains after cache advance"))
            })
            .collect();
        assert_eq!(restored_y, before_y, "cache advance restores shared content at its exact screen rows");

        view.scroll_down(3);
        let after_frame = view.render_typed("", &restored_messages, false, &[]);
        let after_y: Vec<_> = shared_markers
            .iter()
            .map(|marker| {
                after_frame
                    .iter()
                    .position(|row| row.contains(marker))
                    .unwrap_or_else(|| panic!("shared content {marker} remains after wheel movement"))
            })
            .collect();
        for ((before_y, after_y), marker) in before_y.iter().zip(after_y.iter()).zip(shared_markers.iter()) {
            assert_eq!(*after_y, *before_y - 3, "three-row gesture moves {marker} by exactly three rows");
        }
        let post_gesture = view.chat_rendered_anchor(&after).expect("post-gesture anchor captured");
        assert_eq!(post_gesture.id, history_message_id(201));
        assert_eq!(post_gesture.row, 2, "three rows into next card's separator, padding, and text");
    }

    #[test]
    fn rendered_anchor_survives_newer_append_and_oldest_history_eviction() {
        use crate::native_box_style::BoxAppearance;

        let width = 21;
        let height = 16;
        let appearance = BoxAppearance::default();
        let before = projected_anchor_history(90, 130);
        let marker = "anchor-115";
        let seed = anchor_for_marker(&before, "msg-115", marker, width, &appearance);
        let mut view = ChatView::new();
        let before_scroll = place_at_anchor(&mut view, &before, width, &appearance, &seed);
        let ids: Vec<_> = before.iter().map(|(id, _)| id.clone()).collect();
        let messages: Vec<_> = before.iter().map(|(_, message)| message.clone()).collect();
        let captured = view
            .rendered_anchor(width, &messages, &ids, &appearance)
            .expect("before-page anchor captured");
        let before_visible = visible_rows_at_anchor(
            &mut view, &before, width, height, &appearance, &captured, marker,
        );

        let after = projected_anchor_history(100, 140);
        let after_index = after.iter().position(|(id, _)| id == "msg-115").unwrap();
        assert_eq!(after_index + 10, seed.index, "oldest page was evicted");
        let after_scroll = place_at_anchor(&mut view, &after, width, &appearance, &captured);
        assert_ne!(after_scroll, before_scroll, "newer append moves tail-relative window");
        let after_visible = visible_rows_at_anchor(
            &mut view, &after, width, height, &appearance, &captured, marker,
        );
        assert_eq!(before_visible, after_visible, "same rendered transcript stays anchored");
        let ids: Vec<_> = after.iter().map(|(id, _)| id.clone()).collect();
        let messages: Vec<_> = after.iter().map(|(_, message)| message.clone()).collect();
        let restored = view
            .rendered_anchor(width, &messages, &ids, &appearance)
            .expect("after-page anchor restored");
        assert_eq!(restored.id, captured.id);
        assert_eq!(restored.row, captured.row);
        assert_eq!(restored.index, after_index);
    }

    // --- typed roles/parts: caller facts, never inferred from text -------

    /// Roles come from the enum only: user text that literally starts with
    /// `assistant:` stays user text (indented marker block), assistant text
    /// keeps its literal `user:` prefix, and neither creates a group or tool
    /// row. A second user message is the only new group.
    #[test]
    fn typed_roles_are_enums_never_text_prefixes() {
        let mut view = ChatView::new();
        let messages = vec![
            ChatMessage::text(ChatRole::User, "assistant: not the assistant role"),
            ChatMessage::text(ChatRole::Assistant, "user: not a user block"),
            ChatMessage::text(ChatRole::User, "second question"),
        ];
        let rows = view.render_typed_classified(60, 10, "ws", &messages, "", false);
        let user = rows
            .iter()
            .find(|(row, class)| {
                *class == ChatRowClass::UserCard && row.contains("assistant: not the assistant")
            })
            .expect("user text renders as text");
        assert!(
            user.0.starts_with("  assistant:"),
            "user block keeps its left padding: {:?}",
            user.0
        );
        let assistant = rows
            .iter()
            .find(|(row, class)| {
                *class == ChatRowClass::AssistantCard && row.contains("user: not a user block")
            })
            .expect("assistant text renders as text");
        assert!(
            assistant.0.starts_with("user: not a user block"),
            "assistant text is never re-labelled: {:?}",
            assistant.0
        );
        assert_eq!(
            rows.iter()
                .filter(|(_, class)| *class == ChatRowClass::Separator)
                .count(),
            1,
            "only the second user message starts a new group"
        );
        assert!(
            !rows
                .iter()
                .any(|(_, class)| *class == ChatRowClass::ToolCall
                    || *class == ChatRowClass::ToolResult),
            "text prefixes never create tool rows"
        );
    }

    /// Tool part labels and classes come from the typed parts (names, ids,
    /// backend `is_error`), while a user text that looks like a tool header
    /// stays text; thinking rows are their own class.
    #[test]
    fn reasoning_redaction_placeholder_is_not_rendered_as_content() {
        // Upstream's ReasoningPart removes this provider placeholder and trims
        // the remaining summary before rendering it.
        let message = ChatMessage {
            role: ChatRole::Assistant,
            parts: vec![ChatPart::Thinking(
                "  [REDACTED] private reasoning [REDACTED]  ".to_owned(),
            )],
            agent: None,
            model: None,
            assistant_facts: None,
        };
        let rows = ChatView::new().render_typed_classified(48, 6, "ws", &[message], "", false);
        assert!(rows.iter().any(|(row, class)| {
            *class == ChatRowClass::Thinking && row.trim() == "private reasoning"
        }));
        assert!(!rows.iter().any(|(row, _)| row.contains("[REDACTED]")));

        let only_placeholder = ChatMessage {
            role: ChatRole::Assistant,
            parts: vec![ChatPart::Thinking("[REDACTED]".to_owned())],
            agent: None,
            model: None,
            assistant_facts: None,
        };
        let rows =
            ChatView::new().render_typed_classified(48, 6, "ws", &[only_placeholder], "", false);
        assert!(
            !rows
                .iter()
                .any(|(_, class)| *class == ChatRowClass::Thinking)
        );
    }

    #[test]
    fn typed_tool_thinking_and_metadata_rows_are_labeled_from_facts() {
        let mut view = ChatView::new();
        let messages = vec![
            ChatMessage {
                role: ChatRole::User,
                parts: vec![ChatPart::Text("tool: rm -rf /".to_owned())],
                agent: None,
                model: None,
                assistant_facts: None,
            },
            ChatMessage {
                role: ChatRole::Assistant,
                parts: vec![
                    ChatPart::Thinking("weigh options".to_owned()),
                    ChatPart::ToolCall {
                        name: "bash".to_owned(),
                        id: "call-1".to_owned(),
                    },
                    ChatPart::ToolResult {
                        name: "bash".to_owned(),
                        output: "exit 0".to_owned(),
                        is_error: false,
                    },
                    ChatPart::ToolCall {
                        name: "write".to_owned(),
                        id: String::new(),
                    },
                    ChatPart::ToolResult {
                        name: "write".to_owned(),
                        output: "denied by policy".to_owned(),
                        is_error: true,
                    },
                ],
                agent: Some("build".to_owned()),
                model: Some("provider/model-1".to_owned()),
                assistant_facts: None,
            },
        ];
        let rows = view.render_typed_classified(72, 16, "ws", &messages, "", false);
        assert!(
            rows.iter()
                .any(|(row, class)| *class == ChatRowClass::Thinking
                    && row.trim() == "weigh options")
        );
        assert!(
            rows.iter()
                .any(|(row, class)| *class == ChatRowClass::ToolCall
                    && row.trim() == "[tool] bash (call-1)")
        );
        assert!(
            rows.iter().any(
                |(row, class)| *class == ChatRowClass::ToolCall && row.trim() == "[tool] write"
            )
        );
        assert!(rows.iter().any(
            |(row, class)| *class == ChatRowClass::ToolResult && row.trim() == "[result] bash"
        ));
        assert!(
            rows.iter()
                .any(|(row, class)| *class == ChatRowClass::ToolResult && row.trim() == "exit 0")
        );
        assert!(
            rows.iter()
                .any(|(row, class)| *class == ChatRowClass::ToolError
                    && row.trim() == "[error] write")
        );
        assert!(
            rows.iter()
                .any(|(row, class)| *class == ChatRowClass::ToolError
                    && row.trim() == "denied by policy")
        );
        assert!(
            rows.iter()
                .any(|(row, class)| *class == ChatRowClass::Metadata
                    && row.trim() == "agent: build  model: provider/model-1")
        );
        assert!(
            rows.iter()
                .any(|(row, class)| *class == ChatRowClass::UserCard
                    && row.trim() == "tool: rm -rf /")
        );
        assert!(
            !rows
                .iter()
                .any(|(row, class)| *class == ChatRowClass::ToolCall && row.contains("rm -rf")),
            "user text is never promoted to a tool fact"
        );
    }

    /// Hostile sizes stay bounded: parts past the cap are skipped, retained
    /// rows are width-bounded, ANSI/C0 never reach a row, and a multi-megabyte
    /// part is read only as a bounded UTF-8 prefix.
    #[test]
    fn typed_parts_rows_and_inputs_are_bounded_and_sanitized() {
        let mut view = ChatView::new();
        let hostile = format!("\x1b[31mred\x07{}", "x".repeat(1024 * 1024));
        let mut parts: Vec<ChatPart> = vec![ChatPart::Text(hostile)];
        parts.extend((0..MAX_PARTS_PER_MESSAGE * 8).map(|i| ChatPart::Text(format!("part {i}"))));
        let messages = vec![ChatMessage {
            role: ChatRole::Assistant,
            parts,
            agent: Some("agent\nname".to_owned()),
            model: None,
            assistant_facts: None,
        }];
        let rows = view.render_typed_classified(50, MAX_VIEW_ROWS, "ws", &messages, "", false);
        assert!(rows.len() <= MAX_VIEW_ROWS, "frame row cap");
        assert_eq!(
            messages[0].metadata_row().as_deref(),
            Some("  agent: agent name"),
            "metadata is sanitized inline before viewport clipping"
        );
        for (row, _) in &rows {
            assert!(row.chars().count() <= 50, "row exceeds width: {row:?}");
            assert!(!row.contains('\x1b'), "ESC leaked: {row:?}");
            for c in row.chars() {
                assert!(!c.is_control(), "control scalar {c:?} leaked: {row:?}");
            }
        }
        let messages = vec![ChatMessage::text(ChatRole::User, "y".repeat(1024 * 1024))];
        let rows = view.render_typed(40, 8, "ws", &messages, "", false);
        assert_eq!(rows.len(), 8, "huge single part still renders one frame");
    }

    /// Tail following, overscroll, and the sticky-bottom composer keep the
    /// existing `render` contract for typed input: 5000 messages never
    /// materialize the whole history below the window.
    #[test]
    fn typed_window_tail_follows_and_composer_stays_bottom() {
        let mut view = ChatView::new();
        let messages: Vec<ChatMessage> = (0..5000)
            .map(|i| ChatMessage::text(ChatRole::Assistant, format!("entry {i}")))
            .collect();
        let rows = view.render_typed(40, 10, "ws", &messages, "", false);
        assert_eq!(rows.len(), 10);
        assert!(rows.iter().any(|row| row.contains("entry 4999")));
        assert!(
            !rows
                .iter()
                .any(|row| row.trim_start().starts_with("entry 0")),
            "only the tail window is rendered"
        );
        view.scroll_up(MAX_SCROLL_ROWS);
        let rows = view.render_typed(40, 10, "ws", &messages, "", false);
        assert!(rows.iter().any(|row| row.contains("entry 0")));
        view.follow_tail();
        let rows = view.render_typed(40, 10, "ws", &messages, "first\nsecond\nthird", false);
        assert_eq!(rows.last().map(String::as_str), Some("  third"));
        assert!(rows.iter().any(|row| row.starts_with("> first")));
    }

    /// Zero/one-row viewports and huge hostile drafts stay safe.
    #[test]
    fn typed_tiny_viewports_and_hostile_drafts_stay_safe() {
        let mut view = ChatView::new();
        let messages = vec![ChatMessage::text(ChatRole::User, "hi")];
        assert!(
            view.render_typed(80, 0, "ws", &messages, "", false)
                .is_empty()
        );
        assert_eq!(view.render_typed(0, 6, "ws", &messages, "", false).len(), 6);
        let hostile = "line\n".repeat(MAX_DRAFT_BYTES);
        let rows = view.render_typed(40, 10, "ws", &messages, &hostile, true);
        assert_eq!(rows.len(), 10, "draft bound keeps the frame whole");
        assert!(rows.iter().all(|row| row.chars().count() <= 40));
    }

    /// 5000-entry history: bounded window, tail follow, meaningful scroll,
    /// overscroll stays in bounds, follow_tail re-pins.
    #[test]
    fn scroll_5000_entry_history_stays_bounded() {
        let mut view = ChatView::new();
        let h = history(5000);
        // Layout at height 10: header, 8 transcript rows, prompt row.
        let rows = view.render(40, 10, "ws", &h, "", false);
        assert_eq!(rows.len(), 10, "full pane filled");
        assert!(rows[0].contains("ws"), "workspace header first");
        assert_eq!(rows[8], "entry 4999", "tail follows newest");
        assert_eq!(rows[9], "> ", "composer prompt is the bottom row");

        let mut scrolled = ChatView::new();
        scrolled.scroll_up(5);
        let rows = scrolled.render(40, 10, "ws", &h, "", false);
        assert_eq!(rows.len(), 10);
        assert_eq!(rows[8], "entry 4994", "5 rows above tail");
        assert!(rows[0].contains("[+5 up]"), "scroll offset visible");

        // Overscroll past the end clamps to the oldest reachable pane.
        let mut overs = ChatView::new();
        overs.scroll_up(usize::MAX);
        let rows = overs.render(40, 10, "ws", &h, "", false);
        assert!(rows.len() <= 10, "overscroll never exceeds height");
        assert!(rows.iter().any(|r| r == "entry 0"));
        assert_eq!(rows[rows.len() - 1], "> ", "composer survives overscroll");

        // Re-pin restores the tail without touching history.
        overs.follow_tail();
        let rows = overs.render(40, 10, "ws", &h, "", false);
        assert_eq!(rows[8], "entry 4999");
    }

    #[test]
    fn overscroll_render_synchronizes_offset_for_immediate_wheel_down() {
        let history: Vec<String> = (0..20).map(|i| format!("entry {i}")).collect();
        let mut flat = ChatView::new();
        flat.scroll_up(usize::MAX);
        let oldest = flat.render(40, 10, "ws", &history, "", false);
        flat.scroll_down(1);
        let next = flat.render(40, 10, "ws", &history, "", false);
        assert!(oldest.iter().any(|row| row == "entry 0"));
        assert!(next.iter().any(|row| row == "entry 1"));
        flat.scroll_up(usize::MAX);
        let _ = flat.render(40, 10, "ws", &history, "", false);
        let resized_oldest = flat.render(40, 5, "ws", &history, "", false);
        flat.scroll_down(1);
        let resized_next = flat.render(40, 5, "ws", &history, "", false);
        assert!(resized_oldest.iter().any(|row| row == "entry 5"));
        assert!(resized_next.iter().any(|row| row == "entry 6"));

        let messages: Vec<ChatMessage> = (0..20)
            .map(|i| ChatMessage::text(ChatRole::Assistant, format!("entry {i}")))
            .collect();
        let mut typed = ChatView::new();
        typed.scroll_up(usize::MAX);
        let oldest = typed.render_typed(40, 10, "ws", &messages, "", false);
        typed.scroll_down(1);
        let next = typed.render_typed(40, 10, "ws", &messages, "", false);
        assert!(oldest.iter().any(|row| row.contains("entry 0")));
        assert!(next.iter().any(|row| row.contains("entry 1")));

        let mut styled = ChatView::new();
        styled.scroll_up(usize::MAX);
        let oldest = styled.render_styled(
            40, 10, "ws", &messages, "", false,
            &crate::native_box_style::BoxAppearance::default(),
        );
        styled.scroll_down(1);
        let next = styled.render_styled(
            40, 10, "ws", &messages, "", false,
            &crate::native_box_style::BoxAppearance::default(),
        );
        assert!(oldest.iter().any(|(row, _class, _)| row.contains("entry 0")));
        assert!(next.iter().any(|(row, _class, _)| row.contains("entry 1")));
    }

    #[test]
    fn styled_sixty_one_messages_reaches_oldest_and_moves_down_one_row() {
        let messages: Vec<ChatMessage> = (0..61)
            .map(|i| ChatMessage::text(ChatRole::Assistant, format!("entry {i:03}")))
            .collect();
        let appearance = crate::native_box_style::BoxAppearance::default();
        let mut view = ChatView::new();
        for _ in 0..40 {
            view.scroll_up(5);
            let _ = view.render_styled(80, 10, "ws", &messages, "", false, &appearance);
        }
        let oldest = view.render_styled(80, 10, "ws", &messages, "", false, &appearance);
        assert!(oldest.iter().any(|(row, _, _)| row.contains("entry 000")), "oldest styled viewport: {oldest:?}");
        view.scroll_down(1);
        let next = view.render_styled(80, 10, "ws", &messages, "", false, &appearance);
        assert!(next.iter().any(|(row, _, _)| row.contains("entry 001")), "one row down: {next:?}");
    }

    /// Appending history while scrolled does not clear scroll state;
    /// error rows render as plain appended history and only appear at the
    /// tail once re-pinned. Render stays bounded across the append.
    #[test]
    fn append_semantics_preserve_scroll_state() {
        let mut h = history(50);
        let mut view = ChatView::new();
        view.scroll_up(3);
        h.push("error: turn failed".to_owned());
        let rows = view.render(40, 8, "ws", &h, "", false);
        assert_eq!(rows.len(), 8);
        assert!(rows[0].contains("[+3 up]"));
        assert!(
            !rows.iter().any(|r| r.contains("error: turn failed")),
            "appended tail rows stay below a scrolled-up window"
        );
        view.follow_tail();
        let rows = view.render(40, 8, "ws", &h, "", false);
        // Actual height-8 layout: header row 0, history rows 1..=6,
        // composer row 7 — the tail error is the last history row.
        assert_eq!(
            rows[rows.len() - 2],
            "error: turn failed",
            "tail re-pinned to the appended error row"
        );
        assert_eq!(rows[rows.len() - 1], "> ", "composer prompt survives");
        assert!(!rows[0].contains("[+"), "scroll suffix cleared");
    }

    /// Multiline draft: `> ` prompt on the first composer row,
    /// continuation rows below, overflow marked, height respected.
    #[test]
    fn multiline_draft_prompt_and_overflow() {
        let h = history(3);
        let mut view = ChatView::new();
        let rows = view.render(40, 10, "ws", &h, "line one\nline two\nline three", false);
        let prompt = rows
            .iter()
            .position(|r| r.starts_with("> "))
            .expect("prompt row");
        assert_eq!(rows[prompt], "> line one");
        assert_eq!(rows[prompt + 1], "  line two");
        assert_eq!(rows[prompt + 2], "  line three");
        assert!(rows.len() <= 10);

        // More draft lines than the pane half: explicit overflow marker.
        let tall: String = (0..64).map(|i| format!("d{i}\n")).collect();
        let rows = view.render(40, 8, "ws", &h, &tall, false);
        assert_eq!(rows.len(), 8, "height hard bound");
        let prompt = rows
            .iter()
            .position(|r| r.starts_with("> "))
            .expect("prompt row");
        assert_eq!(rows[prompt], "> d0");
        assert!(
            rows.iter().any(|r| r.contains("more line")),
            "overflow marked"
        );
    }

    /// Busy comes from the caller bool only; no provider/model status is
    /// ever rendered.
    #[test]
    fn busy_state_is_caller_honest_and_never_fabricated() {
        let h = history(2);
        let busy_rows = ChatView::new().render(40, 6, "my workspace", &h, "hi", true);
        assert!(busy_rows[0].contains("my workspace"));
        assert!(busy_rows[0].contains("Thinking…"));
        let idle_rows = ChatView::new().render(40, 6, "my workspace", &h, "hi", false);
        assert!(!idle_rows[0].contains("Thinking…"));
        for row in &idle_rows {
            assert!(!row.contains("provider"), "no fake provider status");
            assert!(!row.contains("connected"), "no fake connection status");
        }
    }

    #[test]
    fn borderless_composer_has_no_outline_and_keeps_content_rows() {
        let frame = crate::native_box_style::EffectiveBox {
            border_style: crate::native_box_style::BorderStyle::None,
            preset: crate::native_box_style::RadiusPreset::Rounded,
            radius: 4,
            thickness: 2,
            padding: 2,
        };
        assert!(!frame.has_border());
        let rows = ChatView::new().render_typed_classified_framed(
            40, 12, "ws", &[], "draft", false, frame,
        );
        assert_eq!(rows.len(), 12);
        assert!(rows.iter().all(|(row, _)| {
            !row.chars().any(|ch| "┌┐└┘─│╭╮╰╯═║━┃".contains(ch))
        }));
        assert!(rows.iter().any(|(row, class)| {
            *class == ChatRowClass::Composer && row.contains("> draft")
        }));
    }

    #[test]
    fn borderless_message_bubble_uses_plain_typed_rows_without_outline() {
        let mut appearance = crate::native_box_style::BoxAppearance::default();
        appearance.overrides.insert(
            crate::native_box_style::BoxId::Assistant,
            crate::native_box_style::BoxOverride {
                border_style: Some(crate::native_box_style::BorderStyle::None),
                padding: Some(2),
                ..Default::default()
            },
        );
        let messages = [ChatMessage::text(ChatRole::Assistant, "message bubble")];
        let rows = ChatView::new().render_styled(
            40, 12, "ws", &messages, "", false, &appearance,
        );
        let bubble = rows
            .iter()
            .find(|row| row.0.contains("message bubble"))
            .expect("borderless assistant content row");
        assert!(!bubble.0.chars().any(|ch| "┌┐└┘─│╭╮╰╯═║━┃".contains(ch)));
    }

    #[test]
    fn composer_layout_boundary_uses_plain_mode_at_height_three() {
        let frame = crate::native_box_style::EffectiveBox {
            border_style: crate::native_box_style::BorderStyle::Single,
            preset: crate::native_box_style::RadiusPreset::Sharp,
            radius: 0,
            thickness: 1,
            padding: 0,
        };
        let layout = composer_layout(40, 3, "draft", frame);
        assert!(!layout.framed);
        assert_eq!(layout.height, 1);
        let layout = composer_layout(40, 4, "draft", frame);
        assert!(layout.framed);
        assert!(layout.height >= 3);
    }

    #[test]
    fn framed_direct_render_clamps_extreme_height_before_allocation() {
        let frame = crate::native_box_style::EffectiveBox {
            border_style: crate::native_box_style::BorderStyle::Single,
            preset: crate::native_box_style::RadiusPreset::Sharp,
            radius: 0,
            thickness: 1,
            padding: 0,
        };
        let rows = ChatView::new().render_typed_classified_framed(
            40, usize::MAX, "ws", &[], "draft", false, frame,
        );
        assert_eq!(rows.len(), MAX_VIEW_ROWS);
    }

    /// ANSI, OSC-8 hyperlinks and tabs never cross the render boundary;
    /// CJK/emoji rows stay char-boundary safe under width truncation.
    #[test]
    fn control_sequences_and_unicode_are_sanitized() {
        let h = vec![
            "\x1b[31mred\x1b[0m line".to_owned(),
            "\x1b]8;;http://evil.example/\x1b\\click\x1b]8;;\x1b\\".to_owned(),
            "tab\there".to_owned(),
            "日本語🦀の長い行".to_owned(),
        ];
        let rows = ChatView::new().render(6, 12, "\x1b[1mws\x1b[0m", &h, "", false);
        for row in &rows {
            assert!(!row.contains('\x1b'), "no ESC crosses: {row:?}");
            assert!(!row.contains("evil.example"), "hostile OSC-8 dropped");
            assert!(!row.contains('\t'), "tabs blanked");
        }
        assert!(rows.iter().any(|r| r.contains("red")), "plain text kept");
        // Width bound holds for every row (width > 0).
        for row in rows.iter() {
            assert!(row.chars().count() <= 6, "row too wide: {row:?}");
        }
        // Draft with combining marks and ZWJ renders without panic or ESC.
        let rows = ChatView::new().render(6, 12, "ws", &h, "é👩‍💻", false);
        assert!(rows.iter().any(|r| r.starts_with("> ")));
        assert!(rows.iter().all(|r| !r.contains('\x1b')));
    }

    /// Degenerate heights render in bounds and never panic.
    #[test]
    fn view_bounds_for_tiny_and_oversized_input() {
        let h = history(100);
        for height in [0_usize, 1, 2, 3, 5, 24] {
            let rows = ChatView::new().render(20, height, "ws", &h, "a\nb", false);
            assert!(rows.len() <= height, "height {height} exceeded");
        }
        assert!(
            ChatView::new()
                .render(20, 0, "ws", &h, "", false)
                .is_empty()
        );
        // Empty history still yields header + prompt, no fake filler.
        let rows = ChatView::new().render(20, 6, "ws", &[], "", false);
        assert_eq!(rows[0], "ws");
        assert_eq!(rows[rows.len() - 1], "> ");
        assert!(rows.len() <= 6);
        // width == 0 disables wrapping; row count stays height-bound.
        let rows = ChatView::new().render(0, 4, "ws", &h, "", false);
        assert_eq!(rows.len(), 4);
    }

    /// Regression for the header-composition fix: workspace + busy /
    /// scrolled suffix is clipped as ONE char-safe row, so every header
    /// line (and every other row) stays within `width` even at width
    /// 1/2 with a Unicode workspace. `width == 0` keeps the full header
    /// (no truncation, no underflow). `height == 1` keeps composer
    /// priority: exactly one row, the `"> "` prompt.
    #[test]
    fn header_clips_to_width_with_busy_scrolled_and_unicode_workspace() {
        let h = history(2);
        let workspace = "工作🦀区";
        for width in [1_usize, 2, 3] {
            for busy in [false, true] {
                for scroll in [0_usize, 3] {
                    let mut view = ChatView::new();
                    if scroll > 0 {
                        view.scroll_up(scroll);
                    }
                    let rows = view.render(width, 6, workspace, &h, "", busy);
                    assert!(!rows.is_empty());
                    for row in &rows {
                        assert!(
                            row.chars().count() <= width,
                            "width {width} busy {busy} scroll {scroll} row too wide: {row:?}"
                        );
                    }
                    // rows[0] is the header whenever height >= 2.
                    assert!(
                        rows[0].chars().count() <= width,
                        "header line exceeds width {width}"
                    );
                }
            }
        }
        // width == 0: full header preserved, suffix included, no panic.
        let mut view = ChatView::new();
        view.scroll_up(3);
        let long_h = history(10);
        let rows = view.render(0, 6, workspace, &long_h, "", true);
        assert!(rows[0].contains(workspace), "width 0 keeps workspace");
        assert!(
            rows[0].contains("Thinking…"),
            "width 0 keeps busy label"
        );
        assert!(rows[0].contains("[+3 up]"), "width 0 keeps scroll label");

        // height 1: composer prompt wins; height 0: nothing rendered.
        let rows = ChatView::new().render(20, 1, workspace, &h, "draft", false);
        assert_eq!(rows.len(), 1, "height 1 stays bounded");
        assert!(rows[0].starts_with("> "), "height 1 shows the prompt");
        assert!(
            ChatView::new()
                .render(20, 0, workspace, &h, "draft", false)
                .is_empty()
        );
    }

    /// Empty / short history keeps the composer on the pane's bottom row
    /// and never shifts the prompt under the workspace header: the
    /// allocated transcript viewport materializes as bounded EMPTY rows
    /// (no fabricated filler content), and a short transcript stays
    /// bottom-anchored directly above the composer.
    #[test]
    fn empty_and_short_history_keep_composer_on_the_bottom_row() {
        // Empty history at full shell geometry (120 cols x 30 rows):
        // workspace header row 0, viewport rows 1..=28 blank, prompt row 29.
        let rows = ChatView::new().render(120, 30, "/private/tmp/real-project", &[], "", false);
        assert_eq!(rows.len(), 30, "full pane filled");
        assert_eq!(
            rows[0], "/private/tmp/real-project",
            "workspace header on row 0"
        );
        assert_eq!(rows[29], "> ", "prompt on the bottom row");
        assert!(
            rows[1..29].iter().all(String::is_empty),
            "transcript viewport is blank — no synthesized transcript rows"
        );
        // Few-line transcript: content bottom-anchored directly above the
        // composer, blank viewport above it; prompt still not under the
        // header.
        let h = vec!["first".to_owned(), "second".to_owned()];
        let rows = ChatView::new().render(120, 30, "ws", &h, "", false);
        assert_eq!(rows.len(), 30);
        assert_eq!(rows[27], "first", "oldest visible row above the tail");
        assert_eq!(rows[28], "second", "tail row hugs the composer");
        assert_eq!(rows[29], "> ", "prompt stays on the bottom row");
        assert!(rows[1..27].iter().all(String::is_empty));
    }

    /// Multiline drafts on tiny panes stay bounded to exactly `height`
    /// rows with the prompt inside the composer block (never row 0, never
    /// past the last row), and control sequences never cross the padded
    /// render boundary; every row stays within `width` chars.
    #[test]
    fn padded_viewport_keeps_tiny_multiline_and_sanitization_bounds() {
        for height in [3_usize, 4, 5] {
            let rows = ChatView::new().render(20, height, "ws", &[], "a\nb", false);
            assert_eq!(rows.len(), height, "exact height {height}");
            assert_eq!(rows[0], "ws", "header stays row 0");
            let prompt = rows
                .iter()
                .position(|r| r.starts_with("> "))
                .expect("prompt row");
            assert!(prompt >= 1, "prompt never replaces the header");
            assert!(prompt < height, "prompt stays inside the pane");
            for row in &rows {
                assert!(row.chars().count() <= 20, "row too wide: {row:?}");
            }
        }
        // Empty draft: the prompt sits exactly on the bottom row for every
        // height >= 3 (height 0/1/2 keep their documented priorities).
        for height in [3_usize, 4, 5, 6] {
            let rows = ChatView::new().render(20, height, "ws", &[], "", false);
            assert_eq!(rows.len(), height);
            assert_eq!(rows[height - 1], "> ", "bottom prompt at height {height}");
        }
        // ANSI / OSC-8 / tab never cross the boundary through header,
        // padded viewport or composer rows; width bound holds everywhere.
        let hostile = vec!["\x1b[31mred\x1b[0m line".to_owned()];
        let rows = ChatView::new().render(6, 8, "\x1b[1mws\x1b[0m", &hostile, "", false);
        assert_eq!(rows.len(), 8);
        for row in &rows {
            assert!(!row.contains('\x1b'), "no ESC crosses: {row:?}");
            assert!(row.chars().count() <= 6, "row too wide: {row:?}");
        }
        assert!(rows.iter().any(|r| r.contains("red")), "plain text kept");
        assert_eq!(rows[7], "> ", "prompt on the bottom row");
    }

    /// Reviewer resource bound (general NEEDS_FIX on proposal v1): extreme
    /// public inputs — `width == 0` (no wrap), `usize::MAX` height, one
    /// 8 MiB history row, an 8 MiB newline-heavy draft — stay bounded end
    /// to end: output never exceeds [`MAX_VIEW_ROWS`], every row stays
    /// within the documented byte bounds, no raw ESC leaks through prefix
    /// truncation, and the composer keeps its priority with an explicit
    /// overflow marker (no fake history; nothing silently erased within
    /// valid input). Owned pure fixtures — test only, never live data.
    #[test]
    fn hostile_extreme_inputs_stay_bounded_and_composer_priority_survives() {
        let mut huge_row = String::with_capacity(8 * 1024 * 1024 + 16);
        huge_row.push_str("\x1b[31m");
        huge_row.push_str(&"A".repeat(8 * 1024 * 1024));
        huge_row.push_str("\x1b[0m");
        let history = vec![huge_row];
        let draft = "\n".repeat(8 * 1024 * 1024);
        let max_row_bytes = MAX_INPUT_ROW_BYTES + 64;
        let rows = ChatView::new().render(0, usize::MAX, "ws", &history, &draft, false);
        assert!(
            rows.len() <= MAX_VIEW_ROWS,
            "height clamped: {} rows",
            rows.len()
        );
        assert!(rows.len() >= 3, "header + transcript + composer survive");
        for row in &rows {
            assert!(
                row.len() <= max_row_bytes,
                "row over byte bound: {} bytes",
                row.len()
            );
            assert!(!row.contains('\x1b'), "no raw ESC leaks: {row:?}");
        }
        assert!(
            rows.iter().any(|r| r.starts_with("> ")),
            "composer prompt retained"
        );
        assert!(
            rows.iter().any(|r| r.contains("more line")),
            "draft overflow explicit, never silently erased"
        );
        // height 1 keeps composer priority under the same extreme draft.
        let rows = ChatView::new().render(0, 1, "ws", &history, &draft, false);
        assert_eq!(rows.len(), 1, "height-1 bounded");
        assert!(rows[0].starts_with("> "), "height-1 composer priority");

        // Valid ordinary input at the exact caps renders fully: the bounds
        // are inclusive — they never truncate within valid input.
        let valid = "v".repeat(MAX_DRAFT_BYTES);
        let rows = ChatView::new().render(0, 30, "ws", &[], &valid, false);
        assert_eq!(rows.len(), 30);
        assert_eq!(
            rows[rows.len() - 1],
            format!("> {valid}"),
            "32 KiB draft retained verbatim at width 0"
        );
        let big_ws = "w".repeat(MAX_INPUT_ROW_BYTES);
        let rows = ChatView::new().render(0, 6, &big_ws, &[], "", false);
        assert_eq!(
            rows[0].len(),
            MAX_INPUT_ROW_BYTES,
            "workspace row retained at the input cap"
        );

        // UTF-8 boundary adjust (<= 3 bytes) never splits a scalar.
        let uni = format!("{}{}", "x".repeat(MAX_INPUT_ROW_BYTES - 1), "🦀");
        let clipped = clip_utf8_prefix_bytes(&uni, MAX_INPUT_ROW_BYTES);
        assert!(clipped.len() <= MAX_INPUT_ROW_BYTES);
        assert!(clipped.is_char_boundary(clipped.len()));
        assert!(clipped.ends_with('x'), "multi-byte scalar backed off whole");
    }

    /// Reviewer row-work bound (v2 NEEDS_FIX): the composer input is a
    /// bounded COLLECTION, not a full line materialization — the exact
    /// line count comes from one allocation-free pass over the already
    /// 32 KiB-capped prefix, and at most `composer budget + 1` line
    /// slices are ever collected (the +1 is the overflow-sentinel
    /// witness). The `+N more` figure stays the REAL total, never the
    /// truncated vector length. Calls the production helper directly;
    /// owned pure fixtures, test only.
    #[test]
    fn newline_heavy_draft_collects_only_composer_budget_rows() {
        // 32768 newlines = 32769 exact lines, all inside the byte cap.
        let draft = "\n".repeat(MAX_DRAFT_BYTES);
        let budget = 128; // composer budget at the MAX_VIEW_ROWS cap
        let bounded = bounded_composer_rows(&draft, budget + 1);
        assert_eq!(
            bounded.total_lines,
            MAX_DRAFT_BYTES + 1,
            "exact line count, allocation-free pass"
        );
        assert_eq!(
            bounded.rows.len(),
            budget + 1,
            "collect bounded to budget+1 with sentinel witness"
        );
        assert!(bounded.rows.len() <= budget + 1);

        // Production render path at normal geometry: composer_h = 15 for
        // height 30 — 14 kept draft rows + the exact overflow marker.
        let rows = ChatView::new().render(40, 30, "ws", &[], &draft, false);
        assert_eq!(rows.len(), 30, "exact pane height preserved");
        assert!(
            rows.iter().any(|r| r.starts_with("> ")),
            "prompt row retained"
        );
        assert_eq!(
            rows[rows.len() - 1],
            format!("  ... (+{} more lines)", MAX_DRAFT_BYTES + 1 - (30 / 2),),
            "overflow reports the REAL total, not the bounded vec length"
        );

        // Inclusive single-line boundary unchanged: height 1 prompt keeps
        // the full 32 KiB first (only) line.
        let single = "v".repeat(MAX_DRAFT_BYTES);
        let rows = ChatView::new().render(0, 1, "ws", &[], &single, false);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0],
            format!("> {single}"),
            "height-1 prompt priority with exact 32 KiB line"
        );
    }
}

/// Assistant completion footer consumer slice (upstream `AssistantFooter`,
/// `packages/tui/src/routes/session/index.tsx:2055-2071`): footer rows are
/// appended after the message parts only when real execution facts exist, the
/// legacy metadata row is suppressed in that case, and all old semantic
/// assertions above remain pinned unchanged for fact-less messages.
#[cfg(test)]
mod footer_v3_consumer_tests {
    use super::*;
    use crate::native_assistant_facts::AssistantFacts;

    fn assistant_with_facts(facts: AssistantFacts) -> ChatMessage {
        ChatMessage::text(ChatRole::Assistant, "body").with_assistant_facts(facts)
    }

    #[test]
    fn interruption_footer_appends_after_parts() {
        let facts = AssistantFacts::from_error(Some("cancelled".to_owned()), "turn cancelled".to_owned());
        let message = assistant_with_facts(facts);
        let rows = typed_message_rows(80, &message, false);
        assert_eq!(rows.len(), 2, "prose row then footer row");
        assert!(rows[0].0.contains("body"));
        assert_eq!(rows[1].0, " · interrupted");
        assert_eq!(rows[1].1, ChatRowClass::AssistantFooter);
    }

    #[test]
    fn footer_rows_share_the_message_prefix_like_prose() {
        let facts = AssistantFacts::from_error(Some("cancelled".to_owned()), "turn cancelled".to_owned());
        let message = assistant_with_facts(facts);
        // Rail-card path passes Some("") and adds its own three-column inset
        // to every returned row; footer rows must therefore come back
        // unprefixed exactly like prose rows.
        let rows = typed_message_rows_with_prefix(80, &message, false, Some(""));
        assert_eq!(rows.last().unwrap().0, " · interrupted");
    }

    #[test]
    fn legacy_metadata_row_suppressed_when_facts_present() {
        // With facts present the footer IS the metadata line (upstream
        // AssistantFooter), so the legacy `agent: … model: …` row must not
        // double it. Without facts the legacy row stays untouched.
        let facts = AssistantFacts::from_error(Some("cancelled".to_owned()), "turn cancelled".to_owned());
        let mut message = assistant_with_facts(facts);
        message.agent = Some("build".to_owned());
        message.model = Some("provider/model-1".to_owned());
        let rows = typed_message_rows(80, &message, false);
        assert!(
            !rows.iter().any(|(text, class)| *class == ChatRowClass::Metadata),
            "no legacy metadata row when facts render the footer"
        );
        assert!(rows.iter().any(|(text, _)| text == "Build · provider/model-1 · interrupted"));

        let mut legacy = ChatMessage::text(ChatRole::Assistant, "body");
        legacy.agent = Some("build".to_owned());
        legacy.model = Some("provider/model-1".to_owned());
        let rows = typed_message_rows(80, &legacy, false);
        assert!(rows.iter().any(|(text, class)| {
            *class == ChatRowClass::Metadata && text.contains("agent: build")
        }));
    }

    #[test]
    fn generic_error_footer_rows_keep_class_order() {
        let facts = AssistantFacts::from_error(Some("provider_stream_failed".to_owned()), "provider stream failed".to_owned());
        let mut message = assistant_with_facts(facts);
        message.agent = Some("build".to_owned());
        let rows = typed_message_rows(80, &message, false);
        let classes: Vec<ChatRowClass> = rows.iter().map(|(_, class)| *class).collect();
        assert_eq!(
            classes,
            vec![
                ChatRowClass::AssistantCard,
                ChatRowClass::FooterError,
                ChatRowClass::Blank,
                ChatRowClass::AssistantFooter
            ],
            "prose, error notice, gap, metadata line"
        );
        assert!(rows[1].0.trim_start().starts_with("Error: provider stream failed"));
        assert_eq!(rows[3].0.trim_start(), "Build");
    }

    #[test]
    fn generic_error_without_metadata_fact_renders_notice_after_prose_only() {
        // With no real agent/model/interrupt fact the metadata line is empty:
        // upstream's `marginTop` gap belongs to that line, so the rows are
        // just the prose and the error notice.
        let facts = AssistantFacts::from_error(Some("provider_stream_failed".to_owned()), "provider stream failed".to_owned());
        let message = assistant_with_facts(facts);
        let rows = typed_message_rows(80, &message, false);
        let classes: Vec<ChatRowClass> = rows.iter().map(|(_, class)| *class).collect();
        assert_eq!(classes, vec![ChatRowClass::AssistantCard, ChatRowClass::FooterError]);
    }

    #[test]
    fn cancellation_wording_without_typed_code_keeps_error_rows() {
        // Consumer-level pin of the typed-transport rule: an error whose text
        // merely contains cancellation wording (and carries no typed code)
        // renders the generic error rows — never the interrupted suffix.
        let facts = AssistantFacts::from_error(None, "turn cancelled".to_owned());
        let message = assistant_with_facts(facts);
        let rows = typed_message_rows(80, &message, false);
        assert!(
            rows.iter().any(|(text, class)| {
                *class == ChatRowClass::FooterError && text.trim_start().starts_with("Error: turn cancelled")
            }),
            "error notice present: {rows:?}"
        );
        assert!(
            !rows.iter().any(|(text, _)| text.contains("interrupted")),
            "no interrupted suffix without the typed code"
        );
    }

    #[test]
    fn completed_turn_without_renderable_facts_is_byte_identical_to_no_facts() {
        // Real facts exist (usage/stop_reason) but none render: the typed rows
        // must be exactly the fact-less rows — no raw token count, no
        // placeholder footer, no empty rows.
        let facts = AssistantFacts::from_message_frame(&serde_json::json!({
            "usage": {"output_tokens": 20, "reasoning_tokens": 5},
            "stop_reason": "completed"
        }));
        let message = assistant_with_facts(facts);
        let plain = ChatMessage::text(ChatRole::Assistant, "body");
        assert_eq!(
            typed_message_rows(80, &message, false),
            typed_message_rows(80, &plain, false)
        );
    }

    #[test]
    fn non_assistant_message_with_facts_renders_no_footer() {
        let facts = AssistantFacts::from_error(Some("cancelled".to_owned()), "turn cancelled".to_owned());
        let mut message = assistant_with_facts(facts);
        message.role = ChatRole::User;
        let rows = typed_message_rows(80, &message, false);
        assert!(
            !rows.iter().any(|(_, class)| matches!(class, ChatRowClass::AssistantFooter | ChatRowClass::FooterError))
        );
    }

    #[test]
    fn footer_rows_respect_the_message_row_budget() {
        let facts = AssistantFacts::from_error(Some("provider_stream_failed".to_owned()), "provider stream failed".to_owned());
        let mut message = assistant_with_facts(facts);
        message.agent = Some("build".to_owned());
        message.parts = (0..MAX_PARTS_PER_MESSAGE)
            .map(|i| ChatPart::Text(format!("line {i}")))
            .collect();
        let rows = typed_message_rows(80, &message, false);
        assert!(rows.len() <= MAX_MESSAGE_ROWS, "row cap respected: {}", rows.len());
    }
}
