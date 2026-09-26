//! Run entry body: bounded role+text builder with preview truncation.
//!
//! Evidence: `/home/rashid/projects/opencode/packages/opencode/src/cli/cmd/run/entry.body.ts`
//! at HEAD: `entryBody` maps a cleaned commit text to `RunEntryBody`, with
//! empty content collapsing to `RUN_ENTRY_NONE` (`textBody`/`codeBody`/
//! `markdownBody` return none when content empty, `userBody` when blank).

#![forbid(unsafe_code)]

/// Maximum chars in an entry role.
pub const MAX_ENTRY_ROLE: usize = 16;
/// Maximum chars in entry text (64 KiB chars).
pub const MAX_ENTRY_TEXT: usize = 65536;
/// Maximum chars before preview truncation.
pub const PREVIEW_LEN: usize = 200;

/// Bounded entry body value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryBody {
    /// Role: `user`, `assistant`, or `system`.
    pub role: String,
    /// Body text, capped at [`MAX_ENTRY_TEXT`] chars.
    pub text: String,
}

fn is_role(role: &str) -> bool {
    matches!(role, "user" | "assistant" | "system")
}

/// Build display text for a role+text pair.
/// Mirrors `entry.body.ts` `userBody` prefix: user text gets `> ` marker.
pub fn body_text(role: &str, text: &str) -> Result<String, String> {
    if role.chars().count() > MAX_ENTRY_ROLE {
        return Err(format!("role exceeds {MAX_ENTRY_ROLE} chars"));
    }
    if !is_role(role) {
        return Err(format!("unknown role: {role}"));
    }
    if text.is_empty() {
        return Err("empty text".to_string());
    }
    let clipped: String = text.chars().take(MAX_ENTRY_TEXT).collect();
    if role == "user" {
        Ok(format!("> {clipped}"))
    } else {
        Ok(clipped)
    }
}

/// Short preview: first [`PREVIEW_LEN`] chars plus `...` when longer.
pub fn truncate_preview(text: &str) -> String {
    if text.chars().count() <= PREVIEW_LEN {
        return text.to_string();
    }
    let head: String = text.chars().take(PREVIEW_LEN).collect();
    format!("{head}...")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_ok_prefixes_marker() {
        assert_eq!(body_text("user", "hi").unwrap(), "> hi");
    }

    #[test]
    fn assistant_ok_passthrough() {
        assert_eq!(body_text("assistant", "done").unwrap(), "done");
    }

    #[test]
    fn unknown_role_errs() {
        assert!(body_text("tool", "x").is_err());
    }

    #[test]
    fn empty_text_errs() {
        assert!(body_text("user", "").is_err());
    }

    #[test]
    fn preview_truncates_long() {
        let long = "a".repeat(250);
        let out = truncate_preview(&long);
        assert_eq!(out.len(), PREVIEW_LEN + 3);
        assert!(out.ends_with("..."));
    }

    #[test]
    fn preview_short_untouched() {
        assert_eq!(truncate_preview("hi"), "hi");
    }
}
