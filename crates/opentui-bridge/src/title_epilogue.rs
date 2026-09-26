#![forbid(unsafe_code)]
//! Session title truncation + epilogue line + list-row title.
//!
//! Evidence (TS checkout a0d9b6c, NOT pinned 95daf90):
//! - epilogue path `packages/tui/src/routes/session/index.tsx:202-205`
//!   `Locale.truncate(session()?.title ?? "", 50)` then
//!   `sessionEpilogue({ title, sessionID })`.
//! - truncate `packages/tui/src/util/locale.ts:61-64`
//!   `str.length <= len ? str : str.slice(0, len - 1) + "…"`.
//! - epilogue body `packages/tui/src/util/presentation.ts:29-37`
//!   rows `Session <title>` / `Continue opencode -s <id>` + wordmark.
//! - list rows show FULL title: `dialog-session-list.tsx:246`
//!   (`title: x.title`), rename `dialog-session-rename.tsx:20`
//!   (`value={session()?.title}`), sidebar `sidebar.tsx:53`
//!   (`title={session()!.title}`). No truncation there.
//!
//! Divergence: TS counts UTF-16 units; this counts `char`s (no panic on
//! boundary, same as `crate::locale::truncate`). ANSI wordmark renderer-owned
//! (see `crate::logo_art`); [`epilogue_line`] emits plain text only.
//! Default-title check reused via `crate::session_title`; bound reused via
//! `crate::context_ui::Epilogue`.

use crate::context_ui::Epilogue;
use crate::session_title::is_default_title;

/// Epilogue truncation width (index.tsx:203).
pub const TRUNCATE_TITLE_LEN: usize = 50;

/// TS `Locale.truncate(title, 50)`, char-boundary.
#[must_use]
pub fn truncate_title(title: &str) -> String {
    if title.chars().count() <= TRUNCATE_TITLE_LEN {
        return title.to_string();
    }
    title
        .chars()
        .take(TRUNCATE_TITLE_LEN.saturating_sub(1))
        .collect::<String>()
        + "…"
}

/// Plain-text epilogue line: truncated title, plus trailing detail
/// (e.g. `opencode -s <id>` continue row) when `epilogue` is non-empty.
/// Validated against [`Epilogue`] bound; over-bound falls back to title only.
#[must_use]
pub fn epilogue_line(title: &str, epilogue: Option<&str>) -> String {
    let t = truncate_title(title);
    let tail = epilogue.unwrap_or("").trim();
    if tail.is_empty() {
        return t;
    }
    let full = format!("{t} | {tail}");
    match Epilogue::new(Some(&full)) {
        Some(_) => full,
        None => t,
    }
}

/// Full (untruncated) title for rename/list rows (rename.tsx:20, list.tsx:246).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListTitle(String);

impl ListTitle {
    #[must_use]
    pub fn display(title: &str) -> String {
        Self(title.to_string()).get()
    }
    #[must_use]
    pub fn get(&self) -> String {
        self.0.clone()
    }
    #[must_use]
    pub fn is_default(&self) -> bool {
        is_default_title(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_passthrough() {
        assert_eq!(truncate_title("abcdef"), "abcdef");
        assert_eq!(truncate_title(""), "");
    }

    #[test]
    fn exact_50_passthrough() {
        let s = "a".repeat(50);
        assert_eq!(truncate_title(&s), s);
    }

    #[test]
    fn over_50_gets_ellipsis() {
        // TS: "a"*51.slice(0,49) + "…" == "a"*49 + "…"
        assert_eq!(truncate_title(&"a".repeat(51)), "a".repeat(49) + "…");
        assert_eq!(truncate_title(&"a".repeat(60)), "a".repeat(49) + "…");
    }

    #[test]
    fn unicode_char_boundary() {
        // "é"*60 is 60 chars (120 UTF-16? no, BMP) -> 49 chars + "…", no panic.
        let s = "é".repeat(60);
        let got = truncate_title(&s);
        assert_eq!(got, "é".repeat(49) + "…");
        assert_eq!(got.chars().count(), 50);
        // emoji (surrogate pair in TS): char-based, no split panic.
        let e = "😀".repeat(60);
        assert_eq!(truncate_title(&e).chars().count(), 50);
    }

    #[test]
    fn epilogue_line_shapes() {
        assert_eq!(epilogue_line("hi", None), "hi");
        assert_eq!(epilogue_line("hi", Some("  ")), "hi");
        assert_eq!(
            epilogue_line("hi", Some("opencode -s abc")),
            "hi | opencode -s abc"
        );
        let long = "b".repeat(60);
        assert_eq!(
            epilogue_line(&long, Some("opencode -s abc")),
            "b".repeat(49) + "… | opencode -s abc"
        );
    }

    #[test]
    fn list_title_full_and_default_flag() {
        let long = "c".repeat(200);
        assert_eq!(ListTitle::display(&long), long);
        assert_eq!(ListTitle::display("My session"), "My session");
        assert!(ListTitle("New session - 2026-06-06T12:34:56.789Z".into()).is_default());
        assert!(!ListTitle("My session".into()).is_default());
    }
}
