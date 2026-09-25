#![forbid(unsafe_code)]
//! Route-level side panel (mirrors `routes/session/sidebar.tsx`).
//!
//! TS truth holds session title/workspace/share slots in an overlay box;
//! this type tracks which sidebar section the route shows and whether the
//! panel is open. Row content lives in `crate::sidebar`.

/// Selectable sidebar section (one per `sidebar/*.tsx` plugin).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SideSection {
    #[default]
    Files,
    Context,
    Todo,
    Mcp,
    Lsp,
}

impl SideSection {
    /// Static section label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Files => "Files",
            Self::Context => "Context",
            Self::Todo => "Todo",
            Self::Mcp => "Mcp",
            Self::Lsp => "Lsp",
        }
    }
}

/// Route side panel: open flag + active section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SidePanel {
    pub open: bool,
    pub section: SideSection,
}

impl SidePanel {
    #[must_use]
    pub const fn new(section: SideSection) -> Self {
        Self {
            open: false,
            section,
        }
    }

    /// Flip open flag.
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// Select section and open panel.
    pub fn select(&mut self, section: SideSection) {
        self.section = section;
        self.open = true;
    }

    /// Close panel, keep section.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Active section label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        self.section.label()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_flips() {
        let mut p = SidePanel::new(SideSection::Files);
        assert!(!p.open);
        p.toggle();
        assert!(p.open);
        p.toggle();
        assert!(!p.open);
    }

    #[test]
    fn select_opens() {
        let mut p = SidePanel::new(SideSection::Files);
        p.select(SideSection::Todo);
        assert!(p.open);
        assert_eq!(p.section, SideSection::Todo);
    }

    #[test]
    fn close_keeps_section() {
        let mut p = SidePanel::new(SideSection::Mcp);
        p.select(SideSection::Lsp);
        p.close();
        assert!(!p.open);
        assert_eq!(p.section, SideSection::Lsp);
    }

    #[test]
    fn labels_non_empty() {
        for s in [
            SideSection::Files,
            SideSection::Context,
            SideSection::Todo,
            SideSection::Mcp,
            SideSection::Lsp,
        ] {
            assert!(!s.label().is_empty());
            assert!(!SidePanel::new(s).label().is_empty());
        }
    }

    #[test]
    fn reselect_same_stays_open() {
        let mut p = SidePanel::new(SideSection::Context);
        p.select(SideSection::Context);
        assert!(p.open);
        p.select(SideSection::Context);
        assert!(p.open);
        assert_eq!(p.section, SideSection::Context);
    }

    #[test]
    fn default_closed_on_files() {
        let p = SidePanel::default();
        assert!(!p.open);
        assert_eq!(p.section, SideSection::Files);
    }
}
