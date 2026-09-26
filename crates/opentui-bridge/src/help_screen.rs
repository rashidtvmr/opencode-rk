#![forbid(unsafe_code)]
//! Help screen frame: title + keymap rows + back footer.
//! TS truth `crates/cli/src/tui_entry.rs:552` (`NativePage::Help` arm);
//! rows mirror `crate::keymap_default::describe` output.

use crate::keymap_default::{default_keymap, describe};

/// Screen title.
#[must_use]
pub fn help_title() -> &'static str {
    "Help"
}

fn clip(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        s.to_string()
    } else {
        s.chars().take(width).collect()
    }
}

/// Frame help screen to exactly `height` rows, each char-safe clipped to `width`.
/// Layout: `Help`, `describe` rows, `Esc back` footer always last
/// (height 0 coerced to 1 yields the footer alone).
pub fn help_lines(width: usize, height: usize) -> Vec<String> {
    let width = width.max(1);
    let height = height.max(1);
    let foot = clip("Esc back", width);
    if height == 1 {
        return vec![foot];
    }
    let mut body = vec![help_title().to_string()];
    body.extend(describe(&default_keymap()));
    let mut lines: Vec<String> = body.into_iter().map(|l| clip(&l, width)).collect();
    lines.truncate(height - 1);
    while lines.len() < height - 1 {
        lines.push(String::new());
    }
    lines.push(foot);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_first_exact_height() {
        let out = help_lines(40, 12);
        assert_eq!(out.len(), 12);
        assert_eq!(out[0], "Help");
        assert_eq!(help_title(), "Help");
    }

    #[test]
    fn describe_rows_present() {
        let out = help_lines(80, 12);
        for row in describe(&default_keymap()) {
            assert!(out.contains(&row), "missing {row}");
        }
    }

    #[test]
    fn footer_always_last() {
        let out = help_lines(40, 5);
        assert_eq!(out.last().unwrap(), "Esc back");
        let tiny = help_lines(40, 1);
        assert_eq!(tiny, vec!["Esc back".to_string()]);
    }

    #[test]
    fn pads_short_frame() {
        let out = help_lines(40, 12);
        assert_eq!(out.len(), 12);
        assert!(out[9].is_empty());
    }

    #[test]
    fn width_clip_char_safe() {
        let out = help_lines(5, 12);
        for l in &out {
            assert!(l.chars().count() <= 5, "overflow: {l:?}");
        }
    }
}
