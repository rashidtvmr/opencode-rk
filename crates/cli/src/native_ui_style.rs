#![forbid(unsafe_code)]
//! Shared native typography roles. Terminal cell sizes stay fixed; weight and
//! theme-aware contrast establish the visual hierarchy.
use crate::native_theme_runtime::Slot;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role { Heading, SidebarHeading, Item, Description, Selected, Hint, Error }

pub struct Style {
    pub foreground: Slot,
    pub background: Option<Slot>,
    pub attributes: u32,
}

pub struct Line {
    pub text: String,
    pub role: Role,
    pub detail_from: Option<u16>,
}

#[derive(Clone, Copy, Debug)]
pub struct Region {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub role: Role,
    pub detail_from: Option<u16>,
    pub modal: bool,
}

pub fn style(role: Role) -> Style {
    // OpenTUI c01292fd, packages/core/src/types.ts: TextAttributes.BOLD.
    const BOLD: u32 = 1;
    let (foreground, background, attributes) = match role {
        Role::Heading => (Slot::Primary, None, BOLD),
        Role::SidebarHeading => (Slot::Text, None, BOLD),
        Role::Item => (Slot::Text, None, 0),
        Role::Description | Role::Hint => (Slot::TextMuted, None, 0),
        Role::Selected => (Slot::Background, Some(Slot::Primary), BOLD),
        Role::Error => (Slot::Error, None, BOLD),
    };
    Style { foreground, background, attributes }
}

/// Overlay producers reserve title/hint rows and emit explicit selection
/// gutters. Inspect the gutter before trimming untrusted item labels.
pub fn overlay_roles(lines: &[String], selectable: bool) -> Vec<Role> {
    lines.iter().enumerate().map(|(index, text)| {
        if index == 0 { Role::Heading }
        else if selectable && (text.starts_with("> ") || text.starts_with(" > ")) { Role::Selected }
        else if text.starts_with("Error: ") { Role::Error }
        else if index + 1 == lines.len() { Role::Hint }
        else if text.trim().is_empty() || text.starts_with("Search:") { Role::Description }
        else { Role::Item }
    }).collect()
}

pub fn help_top(rows: u16, content_rows: usize) -> u16 {
    if usize::from(rows) >= content_rows + 6 { 2 }
    else if usize::from(rows) >= content_rows + 2 { 1 }
    else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn headings_are_prominent_and_descriptions_secondary() {
        assert_eq!(style(Role::Heading).attributes, 1);
        assert!(matches!(style(Role::Heading).foreground, Slot::Primary));
        assert!(matches!(style(Role::Description).foreground, Slot::TextMuted));
        assert_eq!(style(Role::Item).attributes, 0);
    }
    #[test]
    fn selection_has_opaque_contrasting_theme_colors_and_weight() {
        let selected = style(Role::Selected);
        assert!(matches!(selected.background, Some(Slot::Primary)));
        assert!(matches!(selected.foreground, Slot::Background));
        assert_eq!(selected.attributes, 1);
    }
    #[test]
    fn hints_and_errors_remain_distinct() {
        assert!(matches!(style(Role::Hint).foreground, Slot::TextMuted));
        assert!(matches!(style(Role::Error).foreground, Slot::Error));
    }
    #[test]
    fn overlay_selection_uses_explicit_gutters_and_never_child_history() {
        let rows = vec!["Picker".into(), "  > unselected label".into(), "> selected label".into(), "Esc close".into()];
        let roles = overlay_roles(&rows, true);
        assert_eq!(roles, vec![Role::Heading, Role::Item, Role::Selected, Role::Hint]);
        assert!(!overlay_roles(&rows, false).contains(&Role::Selected));
    }
    #[test]
    fn compact_help_reduces_margins_before_dropping_keybindings() {
        assert_eq!(help_top(24, 10), 2);
        assert_eq!(help_top(12, 10), 1);
        assert_eq!(help_top(10, 10), 0);
        for rows in 10..40 { assert!(usize::from(rows - 2 * help_top(rows, 10)) >= 10); }
    }
}
