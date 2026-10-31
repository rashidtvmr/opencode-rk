#![forbid(unsafe_code)]
//! Shared native typography roles. Terminal cell sizes stay fixed; weight and
//! theme-aware contrast establish the visual hierarchy.
use crate::native_theme_runtime::Slot;

// ── Text attributes ───────────────────────────────────────────────────
//
// Pinned OpenTUI `TextAttributes` bitmask (`packages/core/src/types.ts`,
// OpenTUI 0.6.8 @ 8d6065a3e60039d16c6ee697692dcfbc46d3c786). The native
// bridge forwards this mask unchanged to `bufferDrawText`, so the numeric
// values are the contract, not an internal choice.
pub const ATTR_NONE: u32 = 0;
pub const ATTR_BOLD: u32 = 1;
pub const ATTR_DIM: u32 = 2;
pub const ATTR_ITALIC: u32 = 4;
pub const ATTR_UNDERLINE: u32 = 8;
pub const ATTR_BLINK: u32 = 16;
pub const ATTR_INVERSE: u32 = 32;
pub const ATTR_HIDDEN: u32 = 64;
pub const ATTR_STRIKETHROUGH: u32 = 128;

/// Every attribute the theme resolver may emit (bold/dim/italic/underline/
/// reverse). Blink, hidden and strikethrough stay renderer-owned.
pub const THEME_ATTRIBUTES: u32 =
    ATTR_BOLD | ATTR_DIM | ATTR_ITALIC | ATTR_UNDERLINE | ATTR_INVERSE;

/// Canonical names in ascending bit order; `reverse` is OpenTUI `INVERSE`.
pub const ATTRIBUTE_NAMES: [(&str, u32); 5] = [
    ("bold", ATTR_BOLD),
    ("dim", ATTR_DIM),
    ("italic", ATTR_ITALIC),
    ("underline", ATTR_UNDERLINE),
    ("reverse", ATTR_INVERSE),
];

/// Parse attribute names into a mask. `inverse` is accepted as an alias of
/// `reverse`; unknown names fail closed (`None`) instead of being ignored.
#[must_use]
pub fn attributes_from_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<u32> {
    let mut mask = ATTR_NONE;
    for name in names {
        let bit = match name {
            "bold" => ATTR_BOLD,
            "dim" => ATTR_DIM,
            "italic" => ATTR_ITALIC,
            "underline" => ATTR_UNDERLINE,
            "reverse" | "inverse" => ATTR_INVERSE,
            _ => return None,
        };
        mask |= bit;
    }
    Some(mask)
}

/// Names of every theme attribute present in `mask`, ascending bit order.
#[must_use]
pub fn attribute_names(mask: u32) -> Vec<&'static str> {
    ATTRIBUTE_NAMES
        .iter()
        .filter(|(_, bit)| mask & bit != 0)
        .map(|(name, _)| *name)
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role { Heading, SidebarHeading, Item, Description, Selected, Hint, Error, Warning, Action }

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
    let (foreground, background, attributes) = match role {
        Role::Heading => (Slot::Primary, None, ATTR_BOLD),
        Role::SidebarHeading => (Slot::Text, None, ATTR_BOLD),
        Role::Item => (Slot::Text, None, ATTR_NONE),
        Role::Description | Role::Hint => (Slot::TextMuted, None, ATTR_NONE),
        Role::Selected => (Slot::Background, Some(Slot::Primary), ATTR_BOLD),
        Role::Error => (Slot::Error, None, ATTR_BOLD),
        Role::Warning => (Slot::Warning, None, ATTR_BOLD),
        Role::Action => (Slot::Text, Some(Slot::BackgroundPanel), ATTR_NONE),
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

    /// THEME-08: pinned OpenTUI `TextAttributes` numeric values.
    #[test]
    fn attribute_bits_match_pinned_opentui() {
        assert_eq!(ATTR_NONE, 0);
        assert_eq!(ATTR_BOLD, 1);
        assert_eq!(ATTR_DIM, 2);
        assert_eq!(ATTR_ITALIC, 4);
        assert_eq!(ATTR_UNDERLINE, 8);
        assert_eq!(ATTR_BLINK, 16);
        assert_eq!(ATTR_INVERSE, 32);
        assert_eq!(ATTR_HIDDEN, 64);
        assert_eq!(ATTR_STRIKETHROUGH, 128);
        assert_eq!(
            THEME_ATTRIBUTES,
            ATTR_BOLD | ATTR_DIM | ATTR_ITALIC | ATTR_UNDERLINE | ATTR_INVERSE
        );
    }

    /// THEME-08: every required direction is expressible and unknown names
    /// fail closed instead of silently dropping an attribute.
    #[test]
    fn attribute_names_roundtrip_and_alias() {
        let mask = attributes_from_names(["bold", "dim", "italic", "underline", "reverse"])
            .expect("known attributes");
        assert_eq!(mask, THEME_ATTRIBUTES);
        assert_eq!(
            attribute_names(mask),
            vec!["bold", "dim", "italic", "underline", "reverse"]
        );
        assert_eq!(
            attributes_from_names(["inverse"]),
            Some(ATTR_INVERSE),
            "inverse aliases reverse"
        );
        assert_eq!(attributes_from_names(["blink"]), None, "fail closed");
        assert_eq!(attribute_names(0), Vec::<&str>::new());
    }
}
