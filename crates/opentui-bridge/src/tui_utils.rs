#![forbid(unsafe_code)]
//! Misc TUI utils (TS checkout a0d9b6c).
//!
//! - `FadeIn`: mirrors `packages/tui/src/util/signal.ts:19-51`
//!   `createFadeIn` (alpha 0->1, reveal-once; 160ms smoothstep, 16ms frames).
//!   Timer-free: caller drives one 16ms frame per [`FadeIn::advance`].
//! - [`describe_terminal`]: mirrors `packages/tui/src/util/system.ts:15-20`.
//!   OS part lives in `crate::system_info` (not repeated here).
//! - Selection keys: mirrors `packages/tui/src/util/selection.ts:46-77`
//!   `handleSelectionKey`. Copy payload itself lives in
//!   `crate::selection::copy_selection`; [`selection_step`] models only the
//!   keep/clear state machine.
//! - [`DestroyFlag`]: mirrors `packages/tui/src/util/renderer.ts:3`
//!   `destroyRenderer` idempotent `isDestroyed` guard (caller must always
//!   clear the terminal title first, as TS does unconditionally).
//! - [`is_record`]: lexical port of `packages/tui/src/util/record.ts:1`
//!   `isRecord` for `&str`: TS tests any value (`object`, non-null,
//!   non-array); here only the JSON-object shape `{...}` on trimmed text,
//!   which already excludes arrays.
//! - [`normalize_prompt_content`]: mirrors
//!   `packages/tui/src/editor.ts:12-24`: strip one trailing `\n`/`\r\n`
//!   only when the body is otherwise newline-free.

/// Reveal-once fade state (`signal.ts:19-51`).
#[derive(Debug, Clone)]
pub struct FadeIn {
    pub shown: bool,
    pub enabled: bool,
    pub alpha: f32,
    revealed: bool,
    elapsed_ms: u64,
}

/// One animation frame (matches TS `setInterval` 16ms).
pub const FADE_FRAME_MS: u64 = 16;
/// Full fade duration (matches TS `/ 160`).
pub const FADE_DURATION_MS: u64 = 160;

fn smoothstep(p: f32) -> f32 {
    p * p * (3.0 - 2.0 * p)
}

impl FadeIn {
    /// `alpha = show ? 1 : 0`, `revealed = show` (TS lines 20-21).
    #[must_use]
    pub fn new(show: bool, enabled: bool) -> Self {
        Self { shown: show, enabled, alpha: if show { 1.0 } else { 0.0 }, revealed: show, elapsed_ms: 0 }
    }

    #[must_use]
    pub fn revealed(&self) -> bool {
        self.revealed
    }

    /// Effect body (TS lines 24-37): hide -> `alpha = 0` (revealed kept);
    /// shown without animation or already revealed -> jump to 1;
    /// otherwise start fade at 0 and drive with [`Self::advance`].
    pub fn set(&mut self, show: bool, enabled: bool) {
        self.shown = show;
        self.enabled = enabled;
        self.elapsed_ms = 0;
        if !show {
            self.alpha = 0.0;
            return;
        }
        if !enabled || self.revealed {
            self.revealed = true;
            self.alpha = 1.0;
            return;
        }
        self.revealed = true;
        self.alpha = 0.0;
    }

    /// Advance one 16ms frame along the smoothstep curve; no-op when
    /// hidden or already complete.
    pub fn advance(&mut self) -> f32 {
        if !self.shown || self.alpha >= 1.0 {
            return self.alpha;
        }
        self.elapsed_ms = self.elapsed_ms.saturating_add(FADE_FRAME_MS);
        let p = (self.elapsed_ms as f32 / FADE_DURATION_MS as f32).min(1.0);
        self.alpha = smoothstep(p);
        self.alpha
    }
}

/// `system.ts:15-20`: `{program}[ {version}][ in tmux| in screen]`.
/// Empty program falls back to `"unknown"` (TS `||`); tmux wins over screen.
#[must_use]
pub fn describe_terminal(
    term_program: Option<&str>,
    term_version: Option<&str>,
    tmux: bool,
    screen: bool,
) -> String {
    let program = term_program.filter(|s| !s.is_empty()).unwrap_or("unknown");
    let version = term_version.filter(|s| !s.is_empty()).map(|v| format!(" {v}")).unwrap_or_default();
    let mux = if tmux { " in tmux" } else if screen { " in screen" } else { "" };
    format!("{program}{version}{mux}")
}

/// Evidenced keys from `selection.ts:55-66` (`ctrl+c`, `escape`);
/// anything else maps to [`SelectionKey::Other`] (TS line 73+ fallthrough).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKey {
    CtrlC,
    Escape,
    Other,
}

/// Minimal selection focus state for the key state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionState {
    pub has_selection: bool,
    pub focus_has_selection: bool,
    pub focus_in_selection: bool,
}

/// `handleSelectionKey` keep/clear machine (`selection.ts:46-77`):
/// no selection -> noop; `ctrl+c`/`escape` -> clear (copy payload, if any,
/// via `crate::selection::copy_selection`); other keys keep only when the
/// focused target has selection and is inside it, else clear.
#[must_use]
pub fn selection_step(state: SelectionState, key: SelectionKey) -> SelectionState {
    if !state.has_selection {
        return state;
    }
    let keep = matches!(key, SelectionKey::Other)
        && state.focus_has_selection
        && state.focus_in_selection;
    SelectionState { has_selection: keep, ..state }
}

/// Idempotent destroy latch (`renderer.ts:3-7`): first call returns `true`
/// (caller destroys); later calls return `false`. Title-clear is the
/// caller's job and happens on every call, as in TS.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DestroyFlag {
    destroyed: bool,
}

impl DestroyFlag {
    #[must_use]
    pub const fn new() -> Self {
        Self { destroyed: false }
    }

    /// Returns `true` exactly once.
    pub fn destroy(&mut self) -> bool {
        if self.destroyed {
            return false;
        }
        self.destroyed = true;
        true
    }
}

/// Lexical `isRecord` for `&str`: trimmed `{...}` shape, len >= 2.
/// Accepts `"{}"` (TS accepts empty objects); rejects arrays/scalars.
#[must_use]
pub fn is_record(s: &str) -> bool {
    let t = s.trim();
    t.len() >= 2 && t.starts_with('{') && t.ends_with('}')
}

/// `editor.ts:12-24`: strip a single trailing `\n`/`\r\n` only if the body
/// contains no other `\n`/`\r`; otherwise return input unchanged.
#[must_use]
pub fn normalize_prompt_content(content: &str) -> String {
    if let Some(body) = content.strip_suffix("\r\n") {
        if !body.contains('\n') && !body.contains('\r') {
            return body.to_string();
        }
        return content.to_string();
    }
    if let Some(body) = content.strip_suffix('\n') {
        if !body.contains('\n') && !body.contains('\r') {
            return body.to_string();
        }
    }
    content.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fadein_reveal_once_second_show_jumps() {
        let mut f = FadeIn::new(false, true);
        assert_eq!(f.alpha, 0.0);
        f.set(true, true);
        assert_eq!(f.alpha, 0.0);
        for _ in 0..10 {
            f.advance();
        }
        assert_eq!(f.alpha, 1.0);
        f.set(false, true);
        assert_eq!(f.alpha, 0.0);
        f.set(true, true);
        assert_eq!(f.alpha, 1.0);
    }

    #[test]
    fn fadein_disabled_or_initial_shown_jumps() {
        let mut f = FadeIn::new(false, false);
        f.set(true, false);
        assert_eq!(f.alpha, 1.0);
        let g = FadeIn::new(true, true);
        assert_eq!(g.alpha, 1.0);
        assert!(g.revealed());
    }

    #[test]
    fn fadein_smoothstep_midpoint() {
        let mut f = FadeIn::new(false, true);
        f.set(true, true);
        for _ in 0..5 {
            f.advance();
        }
        assert!((f.alpha - 0.5).abs() < 1e-6);
    }

    #[test]
    fn terminal_format_and_fallbacks() {
        assert_eq!(describe_terminal(Some("iTerm.app"), Some("3.5"), true, true), "iTerm.app 3.5 in tmux");
        assert_eq!(describe_terminal(None, None, false, true), "unknown in screen");
        assert_eq!(describe_terminal(Some(""), Some(""), false, false), "unknown");
    }

    #[test]
    fn selection_ctrlc_escape_clear_noop_without_selection() {
        let sel = SelectionState { has_selection: true, focus_has_selection: true, focus_in_selection: true };
        assert!(!selection_step(sel, SelectionKey::CtrlC).has_selection);
        assert!(!selection_step(sel, SelectionKey::Escape).has_selection);
        let none = SelectionState { has_selection: false, focus_has_selection: false, focus_in_selection: false };
        assert_eq!(selection_step(none, SelectionKey::Escape), none);
    }

    #[test]
    fn selection_other_keeps_only_when_focus_guarded() {
        let guarded = SelectionState { has_selection: true, focus_has_selection: true, focus_in_selection: true };
        assert!(selection_step(guarded, SelectionKey::Other).has_selection);
        let loose = SelectionState { has_selection: true, focus_has_selection: true, focus_in_selection: false };
        assert!(!selection_step(loose, SelectionKey::Other).has_selection);
    }

    #[test]
    fn destroy_flag_fires_once() {
        let mut d = DestroyFlag::new();
        assert!(d.destroy());
        assert!(!d.destroy());
    }

    #[test]
    fn record_shape_guard() {
        assert!(is_record(r#"{"a":1}"#));
        assert!(is_record("{}"));
        assert!(!is_record(""));
        assert!(!is_record("[1]"));
        assert!(!is_record("hi"));
    }

    #[test]
    fn prompt_normalizes_single_trailing_newline_only() {
        assert_eq!(normalize_prompt_content("hi\n"), "hi");
        assert_eq!(normalize_prompt_content("hi\r\n"), "hi");
        assert_eq!(normalize_prompt_content("a\nb\n"), "a\nb\n");
        assert_eq!(normalize_prompt_content("hi"), "hi");
        assert_eq!(normalize_prompt_content("\n"), "");
    }
}
