#![forbid(unsafe_code)]
//! Subagent dialog state. TS truth: dialog-subagent.tsx (DialogSubagent -> DialogSelect "Subagent Actions").

/// Dialog holding selected subagent action.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SubDlg {
    agent: String,
    picked: bool,
}

impl SubDlg {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn pick(&mut self, name: &str) {
        self.agent = name.chars().take(128).collect();
        self.picked = true;
    }
    pub fn confirm(&mut self) -> bool {
        let ok = self.picked;
        self.picked = false;
        ok
    }
    pub fn agent_of(&self) -> &str {
        &self.agent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pick_sets_agent() {
        let mut d = SubDlg::new();
        d.pick("subagent.view");
        assert!(d.picked);
        assert_eq!(d.agent_of(), "subagent.view");
    }
    #[test]
    fn confirm_consumes() {
        let mut d = SubDlg::new();
        assert!(!d.confirm());
        d.pick("x");
        assert!(d.confirm());
        assert!(!d.confirm());
    }
    #[test]
    fn cap_128_chars() {
        let mut d = SubDlg::new();
        d.pick(&"a".repeat(200));
        assert_eq!(d.agent_of().chars().count(), 128);
    }
}
