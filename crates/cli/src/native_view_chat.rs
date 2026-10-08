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
//!   never cross the render boundary); tabs are blanked here so a paste
//!   cannot shift columns.
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

use crate::native_transcript::{render_lines, strip_ansi, TranscriptLine};

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
    scroll: usize,
}

impl ChatView {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Scroll up (away from the tail) by `lines`, capped at
    /// [`MAX_SCROLL_ROWS`]. No history length is needed: the window is
    /// derived at render time, and an overscroll renders an empty
    /// transcript region rather than panicking or borrowing stale rows.
    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll = self.scroll.saturating_add(lines).min(MAX_SCROLL_ROWS);
    }

    /// Scroll down (toward the tail) by `lines`. Reaching `0` re-pins the
    /// view to the newest history.
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll = self.scroll.saturating_sub(lines);
    }

    /// Re-pin to the tail: the next [`ChatView::render`] shows the newest
    /// transcript rows.
    pub fn follow_tail(&mut self) {
        self.scroll = 0;
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
        if header_h == 1 {
            rows.push(render_header(width, workspace, busy, self.scroll));
        }
        // Bottom-anchored viewport: fill the allocated transcript region
        // with bounded EMPTY rows when the scroll window is short (empty
        // history, short history or overscroll), so the composer always
        // lands on the pane's last rows instead of shifting up under the
        // header. Only blank rows are added here — never invented
        // transcript content — and the window's content and scroll offset
        // selection below are untouched.
        let window = self.transcript_window(width, transcript_h, transcript);
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
    /// is never cloned. Scrolling past the end of history renders no
    /// transcript rows; the state stays intact so scrolling back down
    /// restores the window.
    fn transcript_window(&self, width: usize, height: usize, transcript: &[String]) -> Vec<String> {
        if height == 0 || transcript.is_empty() {
            return Vec::new();
        }
        let target = self.scroll.saturating_add(height);
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
        if self.scroll >= total {
            return Vec::new();
        }
        let end = (total - self.scroll).min(height);
        rev.truncate(end);
        rev
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
    if busy {
        suffix.push_str(" [turn in flight]");
    }
    if scroll > 0 {
        suffix.push_str(&format!(" [+{scroll} up]"));
    }
    let row = format!(
        "{}{suffix}",
        strip_ansi(clip_utf8_prefix_bytes(workspace, MAX_INPUT_ROW_BYTES)).replace('\t', " ")
    );
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
fn sanitize(text: &str) -> String {
    strip_ansi(text).replace('\t', " ")
}

/// Char-boundary safe row truncation; `width == 0` disables it.
fn truncate_row(text: &str, width: usize) -> String {
    if width == 0 || text.chars().count() <= width {
        return text.to_owned();
    }
    text.chars().take(width).collect()
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

// Typed tool/question/subagent formatting is intentionally absent: it
// belongs to the caller, which builds `transcript: &[String]` rows from
// whatever actual public contract it already holds (for example
// `native_transcript` sanitization helpers). This module makes no
// storage-type assumptions and invents no history types.

#[cfg(test)]
mod tests {
    use super::*;

    fn history(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("entry {i}")).collect()
    }

    /// 5000-entry history: bounded window, tail follow, meaningful scroll,
    /// overscroll stays in bounds, follow_tail re-pins.
    #[test]
    fn scroll_5000_entry_history_stays_bounded() {
        let view = ChatView::new();
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

        // Overscroll past the end: empty transcript, bounded pane.
        let mut overs = ChatView::new();
        overs.scroll_up(usize::MAX);
        let rows = overs.render(40, 10, "ws", &h, "", false);
        assert!(rows.len() <= 10, "overscroll never exceeds height");
        assert!(rows.iter().all(|r| !r.starts_with("entry ")));
        assert_eq!(rows[rows.len() - 1], "> ", "composer survives overscroll");

        // Re-pin restores the tail without touching history.
        overs.follow_tail();
        let rows = overs.render(40, 10, "ws", &h, "", false);
        assert_eq!(rows[8], "entry 4999");
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
        let view = ChatView::new();
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
        assert!(busy_rows[0].contains("[turn in flight]"));
        let idle_rows = ChatView::new().render(40, 6, "my workspace", &h, "hi", false);
        assert!(!idle_rows[0].contains("[turn in flight]"));
        for row in &idle_rows {
            assert!(!row.contains("provider"), "no fake provider status");
            assert!(!row.contains("connected"), "no fake connection status");
        }
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
        assert!(ChatView::new()
            .render(20, 0, "ws", &h, "", false)
            .is_empty());
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
        let rows = view.render(0, 6, workspace, &h, "", true);
        assert!(rows[0].contains(workspace), "width 0 keeps workspace");
        assert!(
            rows[0].contains("[turn in flight]"),
            "width 0 keeps busy label"
        );
        assert!(rows[0].contains("[+3 up]"), "width 0 keeps scroll label");

        // height 1: composer prompt wins; height 0: nothing rendered.
        let rows = ChatView::new().render(20, 1, workspace, &h, "draft", false);
        assert_eq!(rows.len(), 1, "height 1 stays bounded");
        assert!(rows[0].starts_with("> "), "height 1 shows the prompt");
        assert!(ChatView::new()
            .render(20, 0, workspace, &h, "draft", false)
            .is_empty());
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
