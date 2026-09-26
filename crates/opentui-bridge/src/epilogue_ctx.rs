#![forbid(unsafe_code)]
//! Epilogue context: footer lines shown below session output.
//!
//! TS truth `packages/tui/src/context/epilogue.tsx:1-6`
//! (`createSimpleContext` holding a `set(value?: string)` setter).
//! Rust port stores rendered lines + visibility flag; rendering is
//! plain `\n`-joined text owned by this module.

/// Max stored lines.
pub const MAX_LINES: usize = 16;
/// Max chars per line.
pub const MAX_LINE_LEN: usize = 512;
/// Max rendered bytes (`render` cap).
pub const MAX_RENDER_BYTES: usize = 8 * 1024;

/// Epilogue footer state.
#[derive(Debug, Clone, Default)]
pub struct EpilogueCtx {
    lines: Vec<String>,
    shown: bool,
}

impl EpilogueCtx {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a line. Truncates to [`MAX_LINE_LEN`] chars.
    /// Returns `false` (drops line) when at [`MAX_LINES`] cap.
    pub fn push(&mut self, line: &str) -> bool {
        if self.lines.len() >= MAX_LINES {
            return false;
        }
        let s: String = line.chars().take(MAX_LINE_LEN).collect();
        self.lines.push(s);
        true
    }

    pub fn show(&mut self) {
        self.shown = true;
    }

    pub fn hide(&mut self) {
        self.shown = false;
    }

    /// Render joined lines, capped at [`MAX_RENDER_BYTES`] bytes.
    /// Hidden context renders `""`.
    #[must_use]
    pub fn render(&self) -> String {
        if !self.shown {
            return String::new();
        }
        let mut out = self.lines.join("\n");
        if out.len() > MAX_RENDER_BYTES {
            let mut end = MAX_RENDER_BYTES;
            while !out.is_char_boundary(end) {
                end -= 1;
            }
            out.truncate(end);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_renders_empty() {
        let mut c = EpilogueCtx::new();
        assert!(c.push("hello"));
        assert_eq!(c.render(), "");
    }

    #[test]
    fn show_renders_lines() {
        let mut c = EpilogueCtx::new();
        c.push("a");
        c.push("b");
        c.show();
        assert_eq!(c.render(), "a\nb");
    }

    #[test]
    fn hide_after_show_empties() {
        let mut c = EpilogueCtx::new();
        c.push("a");
        c.show();
        c.hide();
        assert_eq!(c.render(), "");
    }

    #[test]
    fn line_cap_16() {
        let mut c = EpilogueCtx::new();
        for i in 0..MAX_LINES {
            assert!(c.push(&format!("l{i}")));
        }
        assert!(!c.push("overflow"));
        c.show();
        assert_eq!(c.render().lines().count(), MAX_LINES);
    }

    #[test]
    fn line_truncated_to_512() {
        let mut c = EpilogueCtx::new();
        assert!(c.push(&"x".repeat(600)));
        c.show();
        assert_eq!(c.render().chars().count(), MAX_LINE_LEN);
    }

    #[test]
    fn render_capped_at_8kib() {
        let mut c = EpilogueCtx::new();
        for _ in 0..MAX_LINES {
            assert!(c.push(&"y".repeat(MAX_LINE_LEN)));
        }
        c.show();
        assert!(c.render().len() <= MAX_RENDER_BYTES);
    }
}
