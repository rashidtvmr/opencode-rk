#![forbid(unsafe_code)]

//! Submit assembly over [`PromptCtx`]: normalize, store, submit + draft preview line.

use crate::prompt_ctx::PromptCtx;
use crate::prompt_full::normalize_paste;

/// Normalize `raw` via [`normalize_paste`]; empty -> `None`.
/// Else `set_draft` + `submit`, return `Some(text)`.
pub fn assemble_submit(ctx: &mut PromptCtx, raw: &str) -> Option<String> {
    let text = normalize_paste(raw);
    if text.is_empty() {
        return None;
    }
    ctx.set_draft(&text);
    ctx.submit();
    Some(text)
}

/// Preview `"> {first draft line}"`, clipped to `width` chars (char-safe).
#[must_use]
pub fn draft_line(ctx: &PromptCtx, width: usize) -> String {
    let first = ctx.draft().lines().next().unwrap_or("");
    let line = format!("> {first}");
    line.chars().take(width).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submit_roundtrip_stores_and_clears() {
        let mut c = PromptCtx::new();
        assert_eq!(assemble_submit(&mut c, "hi"), Some("hi".to_string()));
        assert_eq!(c.draft(), "");
        assert_eq!(c.history(), &["hi".to_string()]);
    }

    #[test]
    fn empty_and_ansi_only_none() {
        let mut c = PromptCtx::new();
        assert_eq!(assemble_submit(&mut c, ""), None);
        assert_eq!(assemble_submit(&mut c, "\u{1b}[0m"), None);
        assert!(c.history().is_empty());
    }

    #[test]
    fn crlf_normalized() {
        let mut c = PromptCtx::new();
        assert_eq!(assemble_submit(&mut c, "a\r\nb"), Some("a\nb".to_string()));
        assert_eq!(c.history()[0], "a\nb");
    }

    #[test]
    fn line_prefix() {
        let mut c = PromptCtx::new();
        c.set_draft("hey");
        assert_eq!(draft_line(&c, 80), "> hey");
    }

    #[test]
    fn line_clips_char_safe() {
        let mut c = PromptCtx::new();
        c.set_draft("hello");
        let got = draft_line(&c, 5);
        assert_eq!(got, "> hel");
        let mut d = PromptCtx::new();
        d.set_draft("héllo");
        let got2 = draft_line(&d, 4);
        assert_eq!(got2.chars().count(), 4);
        assert!(got2.starts_with("> h"));
    }

    #[test]
    fn line_zero_width_and_first_line_only() {
        let mut c = PromptCtx::new();
        c.set_draft("one\ntwo");
        assert_eq!(draft_line(&c, 0), "");
        assert_eq!(draft_line(&c, 80), "> one");
    }
}
