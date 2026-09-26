#![forbid(unsafe_code)]
//! Page frame adapter (BRIDGE-PAR-115).
//! Mirrors `crates/cli/src/tui_entry.rs::native_page_lines` menu strings.
//! Chat composes title+status+rule+body-tail+pad+rule+footer; fixed pages
//! return menu lines verbatim, truncated to height. All output char-safe
//! clipped to width. Std-only.

/// Page selector mirroring `NativePage` in `tui_entry.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Chat,
    Palette,
    Context,
    Help,
}

/// Stable label for `page`.
pub fn page_label(page: Page) -> &'static str {
    match page {
        Page::Chat => "chat",
        Page::Palette => "palette",
        Page::Context => "context",
        Page::Help => "help",
    }
}

const FOOTER: &str = "Enter send • Backspace edit • Ctrl+P commands • Ctrl+T context";
const PLACEHOLDER: &str = "Start typing to send a turn.";

fn clip(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        s.to_string()
    } else {
        s.chars().take(width).collect()
    }
}

fn rule(width: usize) -> String {
    "─".repeat(width.max(1).min(120))
}

fn fixed_lines(page: Page) -> Vec<String> {
    let raw: &[&str] = match page {
        Page::Chat => &[],
        Page::Palette => &[
            "Command palette",
            "  /new           New session",
            "  /sessions      Switch/list sessions",
            "  /model         Switch model",
            "  /agents        Switch agent",
            "  /mcps          MCP controls",
            "  /status        Status",
            "  /themes        Theme",
            "  /fork          Fork session",
            "  /undo /redo    Session history actions",
            "  /share         Share session",
            "  /export        Export transcript",
            "  Esc            Back to chat",
        ],
        Page::Context => &[
            "Context / status",
            "daemon: offline",
            "Usage and source-level context populate from live provider events.",
            "Esc returns to chat.",
        ],
        Page::Help => &[
            "Keyboard help",
            "Enter        submit current draft",
            "Backspace    delete previous character",
            "Ctrl+P       command palette",
            "Ctrl+T       context/status page",
            "?            help",
            "Esc          close page",
            "Ctrl+C       quit and restore terminal",
        ],
    };
    raw.iter().map(|s| s.to_string()).collect()
}

/// Frame `page` to exactly `height` rows (or fewer when fixed lines are
/// short), every line char-safe clipped to `width`.
pub fn page_lines(
    page: Page,
    title: &str,
    status: &str,
    body: &[String],
    width: usize,
    height: usize,
) -> Vec<String> {
    let width = width.max(1);
    let height = height.max(1);
    let mut lines: Vec<String> = if page == Page::Chat {
        let mut out = vec![title.to_string(), status.to_string(), rule(width)];
        let rows = height.saturating_sub(7);
        if body.is_empty() {
            out.push(PLACEHOLDER.to_string());
        } else {
            let start = body.len().saturating_sub(rows.max(1));
            out.extend(body[start..].iter().cloned());
        }
        while out.len() < height.saturating_sub(2) {
            out.push(String::new());
        }
        out.push(rule(width));
        out.push(FOOTER.to_string());
        out
    } else {
        fixed_lines(page)
    };
    lines.truncate(height);
    lines.iter().map(|l| clip(l, width)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("line {i}")).collect()
    }

    #[test]
    fn chat_tails_body_to_height_minus_7() {
        let out = page_lines(Page::Chat, "t", "s", &body(10), 40, 12);
        assert_eq!(out.len(), 12);
        assert_eq!(out[0], "t");
        assert!(out.contains(&"line 9".to_string()));
        assert!(!out.contains(&"line 0".to_string()));
    }

    #[test]
    fn chat_empty_body_shows_placeholder() {
        let out = page_lines(Page::Chat, "t", "s", &[], 40, 10);
        assert!(out.iter().any(|l| l == PLACEHOLDER));
    }

    #[test]
    fn palette_matches_tui_entry_strings() {
        let out = page_lines(Page::Palette, "t", "s", &[], 80, 24);
        assert_eq!(out[0], "Command palette");
        assert!(out.iter().any(|l| l.contains("/new")));
        assert!(out.last().unwrap().contains("Back to chat"));
    }

    #[test]
    fn fixed_pages_truncate_to_height() {
        let out = page_lines(Page::Help, "t", "s", &[], 80, 3);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0], "Keyboard help");
    }

    #[test]
    fn width_clip_is_char_safe_on_multibyte() {
        let title = "─".repeat(30) + "ééé tail";
        let out = page_lines(Page::Chat, &title, "s", &[], 10, 10);
        for l in &out {
            assert!(l.chars().count() <= 10, "overflow: {l:?}");
        }
        assert!(out[0].chars().count() <= 10);
    }

    #[test]
    fn labels_cover_all_pages() {
        assert_eq!(page_label(Page::Chat), "chat");
        assert_eq!(page_label(Page::Palette), "palette");
        assert_eq!(page_label(Page::Context), "context");
        assert_eq!(page_label(Page::Help), "help");
    }
}
