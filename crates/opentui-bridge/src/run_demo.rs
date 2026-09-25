//! Demo script transcript lines.
//!
//! Mirrors `demo.ts` `intro()` examples (lines 1112-1116):
//! the five canonical demo prompts shown on entering `--demo` mode.

#![forbid(unsafe_code)]

/// Five fixed demo transcript lines, no IO.
pub fn demo_lines() -> Vec<String> {
    [
        "/permission bash",
        "/question custom",
        "/fmt markdown",
        "/fmt table",
        "/fmt text your custom text",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// True when `text` equals any [`demo_lines`] entry.
pub fn is_demo_text(text: &str) -> bool {
    demo_lines().iter().any(|l| l == text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_five_lines() {
        assert_eq!(demo_lines().len(), 5);
    }

    #[test]
    fn all_lines_non_empty() {
        assert!(demo_lines().iter().all(|l| !l.is_empty()));
    }

    #[test]
    fn matches_each_demo_line() {
        for line in demo_lines() {
            assert!(is_demo_text(&line), "should match {line}");
        }
    }

    #[test]
    fn unknown_text_is_false() {
        assert!(!is_demo_text("/fmt unknown-kind"));
    }

    #[test]
    fn empty_text_is_false() {
        assert!(!is_demo_text(""));
    }
}
