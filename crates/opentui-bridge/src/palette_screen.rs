#![forbid(unsafe_code)]
//! Palette screen rows over [`FooterMenuFull`].
//!
//! TS truth: `footer.command.tsx:1-60` palette panel (fuzzysort-ranked list,
//! `"> "`-style highlight on the cursor row). Divergence: this crate is
//! std-only so ranking is case-insensitive substring in input order, and
//! `FooterMenuFull` keeps items private (only `selected()`/`len()` read),
//! so both helpers render/filter the highlighted row only.

use crate::footer_menu_full::FooterMenuFull;

fn fit(s: &str, width: usize) -> String {
    let mut t: String = s.chars().take(width).collect();
    while t.chars().count() < width {
        t.push(' ');
    }
    t
}

/// Title + highlighted row (`"> label"`), each padded/clipped to `width`,
/// capped at `height` rows.
pub fn palette_lines(menu: &FooterMenuFull, width: usize, height: usize) -> Vec<String> {
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let mut out = vec![fit("Command palette", width)];
    if out.len() >= height {
        out.truncate(height);
        return out;
    }
    match menu.selected() {
        Some(s) => out.push(fit(&format!("> {s}"), width)),
        None if menu.len() == 0 => out.push(fit("(no commands)", width)),
        None => out.push(fit("(closed)", width)),
    }
    out.truncate(height);
    out
}

/// Case-insensitive substring over highlighted row only (items private), cap 32.
pub fn filter_items(menu: &FooterMenuFull, q: &str) -> Vec<String> {
    let needle = q.to_lowercase();
    let mut out = Vec::new();
    if let Some(s) = menu.selected() {
        if needle.is_empty() || s.to_lowercase().contains(&needle) {
            out.push(s.to_string());
        }
    }
    out.truncate(32);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open() -> FooterMenuFull {
        let mut m = FooterMenuFull::new();
        m.add_item("Open File");
        m.add_item("Save All");
        assert!(m.open_menu());
        m
    }

    #[test]
    fn title_padded_to_width() {
        let l = palette_lines(&open(), 20, 5);
        assert_eq!(l[0], fit("Command palette", 20));
        assert_eq!(l[0].chars().count(), 20);
    }

    #[test]
    fn selected_marked_with_prefix() {
        let l = palette_lines(&open(), 20, 5);
        assert!(l[1].starts_with("> Open File"));
    }

    #[test]
    fn closed_menu_shows_placeholder() {
        let mut m = FooterMenuFull::new();
        m.add_item("x");
        let l = palette_lines(&m, 12, 5);
        assert!(l[1].starts_with("(closed)"));
    }

    #[test]
    fn empty_menu_shows_no_commands() {
        let m = FooterMenuFull::new();
        let l = palette_lines(&m, 14, 5);
        assert!(l[1].starts_with("(no commands)"));
    }

    #[test]
    fn width_clips_and_height_caps() {
        let l = palette_lines(&open(), 5, 1);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].chars().count(), 5);
        assert!(palette_lines(&open(), 0, 5).is_empty());
        assert!(palette_lines(&open(), 10, 0).is_empty());
    }

    #[test]
    fn filter_case_insensitive_and_empty() {
        let m = open();
        assert_eq!(filter_items(&m, "open file"), vec!["Open File".to_string()]);
        assert_eq!(filter_items(&m, "OPEN"), vec!["Open File".to_string()]);
        assert_eq!(filter_items(&m, ""), vec!["Open File".to_string()]);
        assert!(filter_items(&m, "zzz").is_empty());
    }

    #[test]
    fn filter_closed_is_empty() {
        let m = FooterMenuFull::new();
        assert!(filter_items(&m, "").is_empty());
    }
}
