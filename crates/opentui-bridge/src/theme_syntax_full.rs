#![forbid(unsafe_code)]
//! Full syntax tag helpers (TS `packages/tui/src/theme/index.ts` SyntaxStyle).
//! ponytail: fixed tag map + bracket paint, no TextMate engine.

/// Syntax tag, kind capped at 32 chars.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxTag {
    kind: String,
}

impl SyntaxTag {
    #[must_use]
    pub fn new(kind: &str) -> Self {
        Self {
            kind: kind.chars().take(32).collect(),
        }
    }

    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }
}

/// Map language id to highlight tag; unknown falls back to `txt`.
#[must_use]
pub fn tag_for(lang: &str) -> &'static str {
    match lang {
        "rs" => "rs",
        "ts" => "ts",
        "tsx" => "tsx",
        "md" => "md",
        "json" => "json",
        "sh" => "sh",
        _ => "txt",
    }
}

/// Paint a token as `[tag]text`, capped at 512 chars total.
#[must_use]
pub fn paint_token(tag: &str, text: &str) -> String {
    let t: String = tag.chars().take(32).collect();
    let out = format!("[{t}]{text}");
    out.chars().take(512).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tag_caps_kind() {
        assert_eq!(SyntaxTag::new(&"a".repeat(40)).kind().chars().count(), 32);
    }
    #[test]
    fn tag_for_known() {
        assert_eq!(tag_for("rs"), "rs");
        assert_eq!(tag_for("tsx"), "tsx");
    }
    #[test]
    fn tag_for_fallback() {
        assert_eq!(tag_for("zzz"), "txt");
    }
    #[test]
    fn paint_format() {
        assert_eq!(paint_token("rs", "fn"), "[rs]fn");
    }
    #[test]
    fn paint_caps() {
        assert_eq!(paint_token("rs", &"x".repeat(600)).chars().count(), 512);
    }
}
