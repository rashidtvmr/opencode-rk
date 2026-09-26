#![forbid(unsafe_code)]

//! Full prompt buffer: paste normalization + slash/mention flags.
//! TS truth: `packages/tui/src/component/prompt/index.tsx:1181`
//! (`text.replace(/\r\n/g, "\n").replace(/\r/g, "\n")`).

/// Max stored chars (8 KiB chars).
pub const MAX_CHARS: usize = 8192;

/// Normalize pasted text: CRLF/CR to LF, strip ANSI CSI (`ESC [ ... letter`), trunc to [`MAX_CHARS`] chars.
pub fn normalize_paste(text: &str) -> String {
    let folded = text.replace("\r\n", "\n").replace('\r', "\n");
    let stripped = strip_ansi_csi(&folded);
    truncate_chars(&stripped, MAX_CHARS)
}

fn strip_ansi_csi(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            let mut j = i + 2;
            while j < bytes.len() && !bytes[j].is_ascii_alphabetic() {
                j += 1;
            }
            if j < bytes.len() {
                j += 1;
            }
            i = j;
        } else {
            let ch = s[i..].chars().next().unwrap_or('\0');
            if ch == '\0' {
                break;
            }
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect()
}

/// Owned prompt buffer with derived slash/mention flags.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PromptFull {
    /// Normalized text, capped at [`MAX_CHARS`] chars.
    pub text: String,
    /// True when trimmed text starts with `/`.
    pub slash: bool,
    /// True when text contains `@`.
    pub mention: bool,
}

impl PromptFull {
    /// Empty buffer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Normalize `text` via [`normalize_paste`] and recompute flags.
    pub fn set_text(&mut self, text: &str) {
        self.text = normalize_paste(text);
        self.slash = self.text.trim_start().starts_with('/');
        self.mention = self.text.contains('@');
    }

    /// True when trimmed text starts with `/`.
    pub fn is_command(&self) -> bool {
        self.slash
    }

    /// True when trimmed text is empty.
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_folds_to_lf() {
        assert_eq!(normalize_paste("a\r\nb"), "a\nb");
    }

    #[test]
    fn lone_cr_folds_to_lf() {
        assert_eq!(normalize_paste("a\rb"), "a\nb");
    }

    #[test]
    fn ansi_csi_stripped() {
        assert_eq!(normalize_paste("\u{1b}[32mok\u{1b}[0m"), "ok");
    }

    #[test]
    fn slash_detected() {
        let mut p = PromptFull::new();
        p.set_text("  /help me");
        assert!(p.slash && p.is_command());
    }

    #[test]
    fn mention_detected() {
        let mut p = PromptFull::new();
        p.set_text("hi @bob");
        assert!(p.mention && !p.is_command());
    }

    #[test]
    fn trunc_caps_at_8kib_chars() {
        let long = "x".repeat(MAX_CHARS + 100);
        let mut p = PromptFull::new();
        p.set_text(&long);
        assert_eq!(p.text.chars().count(), MAX_CHARS);
        assert!(normalize_paste(&long).chars().count() <= MAX_CHARS);
    }
}
