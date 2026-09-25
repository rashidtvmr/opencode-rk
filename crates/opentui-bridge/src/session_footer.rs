#![forbid(unsafe_code)]
//! Session footer slot (mirrors `session/footer.tsx`).
//!
//! Distinct from `run_footer_view::FooterSurface`: this tracks which
//! session-level slot is visible plus liveness.

/// Visible session footer slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FooterSlot {
    #[default]
    Status,
    Prompt,
    Permission,
    Question,
    Subagent,
}

impl FooterSlot {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Prompt => "prompt",
            Self::Permission => "permission",
            Self::Question => "question",
            Self::Subagent => "subagent",
        }
    }
}

/// Session footer: active slot plus busy flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SessionFooter {
    view: FooterSlot,
    busy: bool,
}

impl SessionFooter {
    #[must_use]
    pub const fn new(view: FooterSlot, busy: bool) -> Self {
        Self { view, busy }
    }

    pub fn show(&mut self, slot: FooterSlot) {
        self.view = slot;
    }

    pub fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
    }

    #[must_use]
    pub const fn view(&self) -> FooterSlot {
        self.view
    }

    #[must_use]
    pub const fn is_busy(&self) -> bool {
        self.busy
    }

    #[must_use]
    pub fn render(&self) -> String {
        let state = if self.busy { "busy" } else { "idle" };
        let s = format!("footer {} {}", self.view.as_str(), state);
        s.chars().take(64).collect()
    }

    #[must_use]
    pub const fn is_interactive(&self) -> bool {
        matches!(
            self.view,
            FooterSlot::Prompt | FooterSlot::Permission | FooterSlot::Question
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_status_idle() {
        let f = SessionFooter::default();
        assert_eq!(f.view(), FooterSlot::Status);
        assert!(!f.is_busy());
        assert_eq!(f.render(), "footer status idle");
    }

    #[test]
    fn show_switches_slot() {
        let mut f = SessionFooter::default();
        f.show(FooterSlot::Permission);
        assert_eq!(f.view(), FooterSlot::Permission);
        assert_eq!(f.render(), "footer permission idle");
    }

    #[test]
    fn busy_render() {
        let f = SessionFooter::new(FooterSlot::Prompt, true);
        assert_eq!(f.render(), "footer prompt busy");
    }

    #[test]
    fn interactive_set() {
        for slot in [
            FooterSlot::Prompt,
            FooterSlot::Permission,
            FooterSlot::Question,
        ] {
            assert!(SessionFooter::new(slot, false).is_interactive());
        }
        for slot in [FooterSlot::Status, FooterSlot::Subagent] {
            assert!(!SessionFooter::new(slot, false).is_interactive());
        }
    }

    #[test]
    fn slot_roundtrip() {
        for slot in [
            FooterSlot::Status,
            FooterSlot::Prompt,
            FooterSlot::Permission,
            FooterSlot::Question,
            FooterSlot::Subagent,
        ] {
            let mut f = SessionFooter::default();
            f.show(slot);
            assert_eq!(f.view(), slot);
        }
    }

    #[test]
    fn render_capped_at_64() {
        let f = SessionFooter::new(FooterSlot::Subagent, true);
        assert!(f.render().len() <= 64);
    }
}
