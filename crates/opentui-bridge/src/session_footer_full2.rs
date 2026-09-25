#![forbid(unsafe_code)]
//! Render-counting wrapper over [`assemble_footer`](crate::footer_assemble::assemble_footer).

use crate::footer_assemble::assemble_footer;

/// Footer line plus render counter.
#[derive(Debug, Default)]
pub struct FooterFlow {
    pub slot: String,
    pub items: Vec<String>,
    pub renders: u64,
}

impl FooterFlow {
    pub fn new(slot: &str) -> Self {
        let mut f = Self::default();
        f.set_slot(slot);
        f
    }

    pub fn set_slot(&mut self, slot: &str) {
        self.slot = slot.chars().take(64).collect();
    }

    pub fn push(&mut self, item: &str) -> bool {
        if self.items.len() >= 16 {
            return false;
        }
        self.items.push(item.to_string());
        true
    }

    pub fn render(&mut self, width: usize) -> Vec<String> {
        self.renders = self.renders.saturating_add(1);
        assemble_footer(&self.slot, &self.items, width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_truncates_to_64() {
        let f = FooterFlow::new(&"s".repeat(80));
        assert_eq!(f.slot.chars().count(), 64);
    }

    #[test]
    fn push_caps_at_16() {
        let mut f = FooterFlow::new("dir");
        for i in 0..16 {
            assert!(f.push(&format!("i{i}")));
        }
        assert!(!f.push("overflow"));
        assert_eq!(f.items.len(), 16);
    }

    #[test]
    fn render_bumps_count() {
        let mut f = FooterFlow::new("dir");
        assert_eq!(f.renders, 0);
        f.render(80);
        f.render(80);
        assert_eq!(f.renders, 2);
    }

    #[test]
    fn render_empty_is_slot() {
        let mut f = FooterFlow::new("dir");
        assert_eq!(f.render(80), vec!["dir".to_string()]);
    }

    #[test]
    fn render_joins_and_clips() {
        let mut f = FooterFlow::new("dir");
        f.push("aa");
        f.push("bb");
        assert_eq!(f.render(80), vec!["aa | bb".to_string()]);
        assert_eq!(f.render(3), vec!["aa".to_string()]);
    }
}
