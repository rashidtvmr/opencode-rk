#![forbid(unsafe_code)]
//! Sidebar screen lines (BRIDGE-PAR-191).
//! Mirrors `routes/session/sidebar.tsx`: title slot, 5 plugin sections,
//! footer slot. Rows are `"<label>: <N>"` per panel plus summary first.

use crate::sidebar_panels::{panel_label, SidePanel2, SidebarPanels};

const PANELS: [SidePanel2; 5] = [
    SidePanel2::Files,
    SidePanel2::Context,
    SidePanel2::Todo,
    SidePanel2::Mcp,
    SidePanel2::Lsp,
];

fn clip(s: &str, width: usize) -> String {
    s.chars().take(width).collect()
}

fn pad(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n >= width {
        s.to_string()
    } else {
        let mut out = String::with_capacity(s.len() + width - n);
        out.push_str(s);
        for _ in n..width {
            out.push(' ');
        }
        out
    }
}

/// Summary row + one `"<label>: <N>"` row per panel, clipped to
/// `width` chars and padded/truncated to exactly `height` rows.
#[must_use]
pub fn sidebar_lines(panels: &SidebarPanels, width: usize, height: usize) -> Vec<String> {
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let mut rows: Vec<String> = Vec::with_capacity(1 + PANELS.len());
    rows.push(clip(&panels.summary(), width));
    for p in PANELS {
        let label = panel_label(p);
        rows.push(clip(&format!("{label}: {}", panels.count_of(label)), width));
    }
    rows.truncate(height);
    while rows.len() < height {
        rows.push(pad("", width));
    }
    rows.into_iter()
        .map(|r| pad(&clip(&r, width), width))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panels() -> SidebarPanels {
        let mut s = SidebarPanels::new(SidePanel2::Todo);
        s.set_count("files", 1);
        s.set_count("context", 2);
        s.set_count("todo", 3);
        s.set_count("mcp", 4);
        s.set_count("lsp", 5);
        s
    }

    #[test]
    fn summary_first_then_five() {
        let out = sidebar_lines(&panels(), 40, 10);
        assert_eq!(out.len(), 10);
        assert_eq!(out[0].trim(), "todo 3 items");
        for (i, want) in ["files: 1", "context: 2", "todo: 3", "mcp: 4", "lsp: 5"]
            .iter()
            .enumerate()
        {
            assert_eq!(out[i + 1].trim(), *want);
        }
    }

    #[test]
    fn row_width_clip() {
        let out = sidebar_lines(&panels(), 8, 6);
        assert_eq!(out.len(), 6);
        assert!(out.iter().all(|r| r.chars().count() == 8));
        assert_eq!(out[1], "files: 1");
    }

    #[test]
    fn height_trunc() {
        let out = sidebar_lines(&panels(), 40, 3);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].trim(), "todo 3 items");
        assert_eq!(out[2].trim(), "context: 2");
    }

    #[test]
    fn zero_dims_empty() {
        assert!(sidebar_lines(&panels(), 0, 6).is_empty());
        assert!(sidebar_lines(&panels(), 10, 0).is_empty());
    }

    #[test]
    fn missing_counts_zero() {
        let s = SidebarPanels::new(SidePanel2::Files);
        let out = sidebar_lines(&s, 40, 6);
        assert_eq!(out[0].trim(), "files 0 items");
        assert!(out[1..6].iter().all(|r| r.trim().ends_with(": 0")));
    }
}
