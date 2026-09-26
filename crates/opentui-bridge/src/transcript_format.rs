#![forbid(unsafe_code)]
//! Transcript markdown headers/blocks (mirrors `packages/tui/src/util/transcript.ts` @ a0d9b6c).
//!
//! Pure format-string port; owns no options/part types (`crate::transcript`
//! holds `TranscriptOptions`/`PartKind`, reused not redefined).
//! ponytail: date strings passed preformatted; TS `toLocaleString()` needs a
//! locale DB with no std equivalent. Upgrade when a tz/locale crate is accepted.

use crate::locale::titlecase;

/// TS `formatTranscript` header (`transcript.ts:32-36`):
/// `# {title}\n\n**Session ID:** {id}\n**Created:** {created}\n**Updated:** {updated}\n\n---\n\n`.
/// `created`/`updated` are display strings; TS formats epoch ms via `toLocaleString()`.
#[must_use]
pub fn session_header(title: &str, id: &str, created: &str, updated: &str) -> String {
    format!(
        "# {title}\n\n**Session ID:** {id}\n**Created:** {created}\n**Updated:** {updated}\n\n---\n\n"
    )
}

/// TS `formatAssistantHeader` (`transcript.ts:67-82`).
/// Empty `model` == metadata off -> `## Assistant\n\n` (`:72-74`; no model
/// data on this side, same rule as `transcript::render_entry`).
/// Else `## Assistant ({titlecase(agent)} · {model}[ · {duration}])\n\n`;
/// empty/None duration omitted (TS `:81` falsy-`duration` branch;
/// e.g. created=0, completed=1500 -> `Some("1.5s")`).
#[must_use]
pub fn assistant_header(agent: &str, model: &str, duration_s: Option<&str>) -> String {
    if model.is_empty() {
        return "## Assistant\n\n".to_string();
    }
    let mut out = format!("## Assistant ({} · {model}", titlecase(agent));
    if let Some(d) = duration_s {
        if !d.is_empty() {
            out.push_str(" · ");
            out.push_str(d);
        }
    }
    out.push_str(")\n\n");
    out
}

/// TS `formatPart` tool input/output/error blocks (`transcript.ts:98-106`).
/// `None`/empty input == details off (`:98` truthy-`input` guard).
/// Output only under `status == "completed"` (`:101`), error only under
/// `status == "error"` (`:104`); status strings verbatim, no enum mapping
/// (`transcript::ToolState::Failed` labels `"error"` verbatim per TS).
/// Covers `:98-106` only; trailing `\n` (`:107`) belongs to `formatPart`.
#[must_use]
pub fn tool_block(
    input: Option<&str>,
    output: Option<&str>,
    error: Option<&str>,
    status: &str,
) -> String {
    let mut out = String::new();
    if let Some(i) = input {
        if !i.is_empty() {
            out.push_str(&format!("\n**Input:**\n```json\n{i}\n```\n"));
        }
    }
    if status == "completed" {
        if let Some(o) = output {
            if !o.is_empty() {
                out.push_str(&format!("\n**Output:**\n```\n{o}\n```\n"));
            }
        }
    }
    if status == "error" {
        if let Some(e) = error {
            if !e.is_empty() {
                out.push_str(&format!("\n**Error:**\n```\n{e}\n```\n"));
            }
        }
    }
    out
}

/// TS `formatPart` text gate (`transcript.ts:85`) + `index.tsx:1362`:
/// text renders only when `!synthetic`. True = skip text render.
#[must_use]
pub const fn is_synthetic(part_flag: bool) -> bool {
    part_flag
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_header_exact() {
        assert_eq!(
            session_header("My Session", "abc-123", "C", "U"),
            "# My Session\n\n**Session ID:** abc-123\n**Created:** C\n**Updated:** U\n\n---\n\n"
        );
    }

    #[test]
    fn assistant_plain_without_model() {
        assert_eq!(assistant_header("", "", None), "## Assistant\n\n");
    }

    #[test]
    fn assistant_with_model_no_duration() {
        assert_eq!(assistant_header("shell", "Claude", None), "## Assistant (Shell · Claude)\n\n");
    }

    #[test]
    fn assistant_with_duration() {
        assert_eq!(
            assistant_header("build agent", "Claude Opus", Some("1.5s")),
            "## Assistant (Build Agent · Claude Opus · 1.5s)\n\n"
        );
    }

    #[test]
    fn assistant_empty_duration_omitted() {
        assert_eq!(
            assistant_header("shell", "Claude", Some("")),
            "## Assistant (Shell · Claude)\n\n"
        );
    }

    #[test]
    fn tool_input_block_exact() {
        assert_eq!(
            tool_block(Some("{\"a\": 1}"), None, None, "running"),
            "\n**Input:**\n```json\n{\"a\": 1}\n```\n"
        );
    }

    #[test]
    fn tool_output_only_when_completed() {
        assert_eq!(tool_block(None, Some("ok"), None, "completed"), "\n**Output:**\n```\nok\n```\n");
        assert_eq!(tool_block(None, Some("ok"), None, "running"), "");
    }

    #[test]
    fn tool_error_only_on_error() {
        assert_eq!(tool_block(None, None, Some("boom"), "error"), "\n**Error:**\n```\nboom\n```\n");
        assert_eq!(tool_block(None, None, Some("boom"), "completed"), "");
    }

    #[test]
    fn synthetic_gate() {
        assert!(is_synthetic(true));
        assert!(!is_synthetic(false));
    }
}
