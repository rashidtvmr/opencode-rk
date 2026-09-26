#![forbid(unsafe_code)]
//! Focused side-panel selector with per-panel item counts.

/// Panels mirrored from `crate::sidebar::Panel` (distinct name avoids clash).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidePanel2 {
    #[default]
    Files,
    Context,
    Todo,
    Mcp,
    Lsp,
}

/// Static label per panel.
#[must_use]
pub const fn panel_label(p: SidePanel2) -> &'static str {
    match p {
        SidePanel2::Files => "files",
        SidePanel2::Context => "context",
        SidePanel2::Todo => "todo",
        SidePanel2::Mcp => "mcp",
        SidePanel2::Lsp => "lsp",
    }
}

/// Name length bound and max count entries (fail-closed).
pub const MAX_NAME: usize = 32;
pub const MAX_PANELS: usize = 8;

/// Active panel plus bounded per-panel counts.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SidebarPanels {
    pub active: SidePanel2,
    pub counts: Vec<(String, u32)>,
}

impl SidebarPanels {
    #[must_use]
    pub const fn new(active: SidePanel2) -> Self {
        Self {
            active,
            counts: Vec::new(),
        }
    }

    fn trunc(name: &str) -> String {
        name.chars().take(MAX_NAME).collect()
    }

    /// Insert or overwrite count; ignores new names when full.
    pub fn set_count(&mut self, name: &str, n: u32) {
        let key = Self::trunc(name);
        if let Some(slot) = self.counts.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = n;
            return;
        }
        if self.counts.len() < MAX_PANELS {
            self.counts.push((key, n));
        }
    }

    /// Count for name, 0 when missing.
    #[must_use]
    pub fn count_of(&self, name: &str) -> u32 {
        let key = Self::trunc(name);
        self.counts
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, n)| *n)
            .unwrap_or(0)
    }

    /// `"<panel> <N> items"` for the active panel.
    #[must_use]
    pub fn summary(&self) -> String {
        let label = panel_label(self.active);
        format!("{} {} items", label, self.count_of(label))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(panel_label(SidePanel2::Files), "files");
        assert_eq!(panel_label(SidePanel2::Context), "context");
        assert_eq!(panel_label(SidePanel2::Todo), "todo");
        assert_eq!(panel_label(SidePanel2::Mcp), "mcp");
        assert_eq!(panel_label(SidePanel2::Lsp), "lsp");
    }

    #[test]
    fn missing_is_zero() {
        let s = SidebarPanels::new(SidePanel2::Files);
        assert_eq!(s.count_of("files"), 0);
        assert_eq!(s.count_of("nope"), 0);
    }

    #[test]
    fn set_and_overwrite() {
        let mut s = SidebarPanels::new(SidePanel2::Todo);
        s.set_count("todo", 2);
        assert_eq!(s.count_of("todo"), 2);
        s.set_count("todo", 5);
        assert_eq!(s.count_of("todo"), 5);
    }

    #[test]
    fn summary_parts() {
        let mut s = SidebarPanels::new(SidePanel2::Mcp);
        s.set_count("mcp", 3);
        let out = s.summary();
        assert!(out.contains("mcp") && out.contains('3') && out.contains("items"));
        assert_eq!(out, "mcp 3 items");
    }

    #[test]
    fn name_truncates() {
        let mut s = SidebarPanels::new(SidePanel2::Files);
        let long = "f".repeat(MAX_NAME + 10);
        s.set_count(&long, 1);
        assert_eq!(s.counts.len(), 1);
        assert_eq!(s.counts[0].0.len(), MAX_NAME);
        assert_eq!(s.count_of(&long), 1);
    }

    #[test]
    fn vec_caps_at_eight() {
        let mut s = SidebarPanels::new(SidePanel2::Files);
        for i in 0..MAX_PANELS + 4 {
            s.set_count(&format!("p{i}"), i as u32);
        }
        assert_eq!(s.counts.len(), MAX_PANELS);
        assert_eq!(s.count_of("p0"), 0);
    }
}
