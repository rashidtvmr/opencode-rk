#![forbid(unsafe_code)]
//! Context + LSP side-panel state.
//! Port of `sidebar/context.tsx` (context entries) and `sidebar/lsp.tsx` (LSP line).

pub const MAX_CTX: usize = 16;
pub const MAX_ITEM: usize = 128;
pub const MAX_LSP: usize = 128;
pub const MAX_SUMMARY: usize = 256;

/// Bounded context entries plus LSP status line.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SidePanels {
    pub ctx: Vec<String>,
    pub lsp: String,
}

impl SidePanels {
    fn trunc(s: &str, n: usize) -> String {
        s.chars().take(n).collect()
    }

    /// Push entry; ignored when full.
    pub fn add_ctx(&mut self, item: &str) {
        if self.ctx.len() >= MAX_CTX {
            return;
        }
        self.ctx.push(Self::trunc(item, MAX_ITEM));
    }

    /// Replace LSP line, truncated.
    pub fn set_lsp(&mut self, v: &str) {
        self.lsp = Self::trunc(v, MAX_LSP);
    }

    /// `"ctx N lsp X"`, capped.
    #[must_use]
    pub fn summary(&self) -> String {
        let s = format!("ctx {} lsp {}", self.ctx.len(), self.lsp);
        Self::trunc(&s, MAX_SUMMARY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_caps_at_16() {
        let mut p = SidePanels::default();
        for i in 0..20 {
            p.add_ctx(&format!("c{i}"));
        }
        assert_eq!(p.ctx.len(), MAX_CTX);
    }

    #[test]
    fn item_truncates() {
        let mut p = SidePanels::default();
        p.add_ctx(&"x".repeat(200));
        assert_eq!(p.ctx[0].chars().count(), MAX_ITEM);
    }

    #[test]
    fn lsp_truncates() {
        let mut p = SidePanels::default();
        p.set_lsp(&"y".repeat(200));
        assert_eq!(p.lsp.chars().count(), MAX_LSP);
    }

    #[test]
    fn summary_caps() {
        let mut p = SidePanels::default();
        p.add_ctx("a");
        p.set_lsp("rust-analyzer ok");
        let s = p.summary();
        assert!(s.contains("ctx 1") && s.contains("rust-analyzer"));
        assert!(s.chars().count() <= MAX_SUMMARY);
    }
}
