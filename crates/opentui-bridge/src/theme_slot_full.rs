#![forbid(unsafe_code)]
//! Full 4-slot theme table.
//!
//! | Slot | `theme_slot` | `border_chars` | `selected_fallback` | `pack_options` |
//! |---|---|---|---|---|
//! | Text | `text` | single | `text` | bit empty |
//! | Muted | `text_muted` | single | `text` | bit empty |
//! | Border | `border` | style table | `text` | bit set |
//! | Selected | `selected_list_item_text` | single | `text` | bit empty |

/// Four canonical slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeSlot {
    Text,
    Muted,
    Border,
    Selected,
}

impl ThemeSlot {
    /// Canonical name for [`crate::theme_utils::theme_slot`].
    #[must_use]
    pub fn slot_name(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Muted => "text_muted",
            Self::Border => "border",
            Self::Selected => "selected_list_item_text",
        }
    }

    /// Degrade chain, self first, rest `Text`.
    #[must_use]
    pub fn fallback_chain(&self) -> [Self; 4] {
        match self {
            Self::Text => [Self::Text; 4],
            Self::Muted => [Self::Muted, Self::Text, Self::Text, Self::Text],
            Self::Border => [Self::Border, Self::Text, Self::Text, Self::Text],
            Self::Selected => [Self::Selected, Self::Text, Self::Text, Self::Text],
        }
    }
}

/// Pack border flag into bit 0.
#[must_use]
pub fn pack_options(border: bool) -> u8 {
    border as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_utils() {
        assert_eq!(ThemeSlot::Text.slot_name(), "text");
        assert_eq!(ThemeSlot::Muted.slot_name(), "text_muted");
        assert_eq!(ThemeSlot::Border.slot_name(), "border");
        assert_eq!(ThemeSlot::Selected.slot_name(), "selected_list_item_text");
    }

    #[test]
    fn chain_self_first_text_last() {
        for s in [
            ThemeSlot::Text,
            ThemeSlot::Muted,
            ThemeSlot::Border,
            ThemeSlot::Selected,
        ] {
            let c = s.fallback_chain();
            assert_eq!(c[0], s);
            assert_eq!(c[3], ThemeSlot::Text);
        }
    }

    #[test]
    fn pack_border_bit() {
        assert_eq!(pack_options(false), 0);
        assert_eq!(pack_options(true), 1);
    }

    #[test]
    fn chain_len_four() {
        assert_eq!(ThemeSlot::Selected.fallback_chain().len(), 4);
    }
}
