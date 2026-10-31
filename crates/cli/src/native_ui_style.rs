#![forbid(unsafe_code)]
//! Shared native typography roles. Terminal cell sizes stay fixed; weight and
//! theme-aware contrast establish the visual hierarchy.
//!
//! A [`Line`] carries one [`Role`] for the whole row, which is enough for the
//! single-`<text>` surfaces but not for the rows upstream paints per span.
//! [`LineSpan`] is the additive per-cell companion: producers return
//! `Vec<(row_index, LineSpan)>` alongside the rows they already emit, so no
//! existing `Line { text, role, detail_from }` producer has to change.
//!
//! Pinned upstream evidence (OpenCode `450e7b92412c780a7b5a75226bb8d49aaca24e22`):
//! * `packages/tui/src/feature-plugins/sidebar/mcp.tsx:62-85` — the status
//!   bullet is its own `<text fg={dot(status)}>`, the server name is
//!   `<b>{name}</b>`, and the status word is `theme.text.muted` unless the
//!   server failed (`theme.text.feedback.error.base`);
//! * `packages/tui/src/feature-plugins/sidebar/mcp.tsx:36-47` and
//!   `feature-plugins/sidebar/todo.tsx:30-40` — a base-text arrow, a bold
//!   section name and a muted collapsed summary inside one visual row;
//! * `packages/tui/src/component/todo-item.tsx:5-21` — the marker and the
//!   content share one status foreground, and a cancelled todo additionally
//!   carries `TextAttributes.STRIKETHROUGH`;
//! * `packages/tui/src/feature-plugins/sidebar/context.tsx:23-37` and
//!   `feature-plugins/sidebar/limit.tsx:27-32` — bold base-text heading rows
//!   followed by muted value rows.
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

impl Line {
    /// Convenience constructor for producers that do not need `detail_from`.
    ///
    /// `Line` deliberately keeps exactly its three historical fields: adding a
    /// fourth would break every struct-literal producer in the crate
    /// (`native_view.rs`, `native_view_pages.rs`, `native_view_dialogs.rs`,
    /// `native_permissions.rs`, `native_command_palette.rs`, ...). Per-span
    /// overrides travel beside the rows instead — see [`LineSpan`] and
    /// `StatusView::sidebar_span_styles`.
    #[must_use]
    pub fn new(text: impl Into<String>, role: Role) -> Self {
        Self {
            text: text.into(),
            role,
            detail_from: None,
        }
    }
}

/// One painted cell range inside a rendered row.
///
/// `start_cell`/`cells` are display cells counted from the first painted cell
/// of the row — the same convention as `native_context::UsageStyleSpan` — and
/// `slot`/`attributes` are the upstream `fg` and `TextAttributes` overrides for
/// exactly those cells. `attributes` uses the pinned OpenTUI bitmask defined
/// above, so a producer can express `STRIKETHROUGH` even though
/// [`THEME_ATTRIBUTES`] (the theme resolver's mask) never emits it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineSpan {
    pub start_cell: usize,
    pub cells: usize,
    pub slot: Slot,
    pub attributes: u32,
}

impl LineSpan {
    #[must_use]
    pub const fn new(start_cell: usize, cells: usize, slot: Slot, attributes: u32) -> Self {
        Self {
            start_cell,
            cells,
            slot,
            attributes,
        }
    }

    /// First cell after this span.
    #[must_use]
    pub const fn end_cell(self) -> usize {
        self.start_cell.saturating_add(self.cells)
    }

    /// Whether `cell` falls inside this span. Empty spans cover nothing.
    #[must_use]
    pub const fn covers(self, cell: usize) -> bool {
        self.cells > 0 && cell >= self.start_cell && cell < self.end_cell()
    }

    /// This span limited to a row of `width` cells; `None` when it starts at or
    /// past the row end or loses all of its cells.
    #[must_use]
    pub const fn clipped(self, width: usize) -> Option<Self> {
        if self.start_cell >= width || self.cells == 0 {
            return None;
        }
        let cells = if self.end_cell() > width {
            width - self.start_cell
        } else {
            self.cells
        };
        Some(Self {
            start_cell: self.start_cell,
            cells,
            slot: self.slot,
            attributes: self.attributes,
        })
    }
}

/// Override in effect at `cell`, if any. Spans are painted in order, so a later
/// span wins where spans overlap (an inner `<span>` over its parent `<text>`).
#[must_use]
pub fn span_at(spans: &[LineSpan], cell: usize) -> Option<LineSpan> {
    spans.iter().rev().copied().find(|span| span.covers(cell))
}

/// Limit every span to a row of `width` cells, dropping the ones that fall
/// outside it. Producers must apply this after any row truncation, otherwise a
/// span can claim cells the painted row no longer has.
#[must_use]
pub fn clamp_spans(spans: &[LineSpan], width: usize) -> Vec<LineSpan> {
    spans.iter().filter_map(|span| span.clipped(width)).collect()
}

/// Row-indexed span table for the painter: every entry addresses one row, and
/// out-of-range or duplicate-row entries are still kept in input order.
#[must_use]
pub fn spans_by_row(rows: usize, spans: &[(usize, LineSpan)]) -> Vec<Vec<LineSpan>> {
    let mut table = vec![Vec::new(); rows];
    for (row, span) in spans {
        if let Some(target) = table.get_mut(*row) {
            target.push(*span);
        }
    }
    table
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

    /// SIDE-07/SIDE-10: the span model is additive — the historical
    /// three-field `Line` literal still compiles with no `spans` field, and
    /// spans live beside the rows.
    #[test]
    fn line_stays_three_fields_and_spans_are_additive() {
        let line = Line {
            text: "• alpha Connected".into(),
            role: Role::Item,
            detail_from: None,
        };
        assert_eq!(line.text, "• alpha Connected");
        assert_eq!(line.role, Role::Item);
        assert_eq!(line.detail_from, None);
        assert_eq!(Line::new("Heading", Role::Heading).text, "Heading");
        // No spans means "paint the row with its role", never a hidden style.
        assert!(spans_by_row(2, &[]).iter().all(Vec::is_empty));
    }

    /// SIDE-07: dot/name/status spans are addressable by display cell, a later
    /// nested span overrides its parent, and truncation clamps cell ranges.
    #[test]
    fn spans_address_display_cells_and_clip_to_the_row() {
        let dot = LineSpan::new(0, 1, Slot::Success, ATTR_NONE);
        let name = LineSpan::new(2, 5, Slot::Text, ATTR_BOLD);
        let status = LineSpan::new(8, 9, Slot::TextMuted, ATTR_NONE);
        let spans = [dot, name, status];
        assert!(dot.covers(0) && !dot.covers(1));
        assert_eq!(name.end_cell(), 7);
        assert_eq!(span_at(&spans, 2), Some(name));
        assert_eq!(span_at(&spans, 7), None, "gap cell has no span");
        assert_eq!(span_at(&spans, 16), Some(status));

        // A 12-cell row keeps the dot and name and cuts the status to 4 cells;
        // a span starting outside the row disappears entirely.
        let clamped = clamp_spans(&spans, 12);
        assert_eq!(clamped, [dot, name, LineSpan::new(8, 4, Slot::TextMuted, ATTR_NONE)]);
        assert_eq!(LineSpan::new(0, 3, Slot::Text, ATTR_NONE).clipped(0), None);
        assert_eq!(LineSpan::new(4, 0, Slot::Text, ATTR_NONE).clipped(10), None);
        // Empty rows and out-of-range rows never gain spans.
        let table = spans_by_row(2, &[(1, dot), (9, name)]);
        assert_eq!(table, vec![Vec::new(), vec![dot]]);
    }

    /// SIDE-10: strikethrough is expressible per span even though the theme
    /// resolver's mask ([`THEME_ATTRIBUTES`]) never emits it.
    #[test]
    fn strikethrough_is_a_span_attribute_outside_the_theme_mask() {
        let cancelled = LineSpan::new(4, 6, Slot::TextMuted, ATTR_STRIKETHROUGH);
        assert_eq!(cancelled.attributes & ATTR_STRIKETHROUGH, ATTR_STRIKETHROUGH);
        assert_eq!(THEME_ATTRIBUTES & ATTR_STRIKETHROUGH, 0);
    }
}
