//! Help-overlay visibility context.
//!
//! Mirrors `packages/tui/src/context/helper.tsx` (`createSimpleContext` show-gate).
//! TS truth is generic with no fixed state; this port fixes the help shape per spec.

#![forbid(unsafe_code)]

/// Max topic chars kept; longer input truncates.
pub const MAX_TOPIC_LEN: usize = 64;

/// Help overlay: visibility + active topic. Hidden by default.
#[derive(Debug, Clone, Default)]
pub struct HelperCtx {
    pub visible: bool,
    pub topic: String,
}

impl HelperCtx {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn cap(topic: &str) -> String {
        if topic.chars().count() > MAX_TOPIC_LEN {
            topic.chars().take(MAX_TOPIC_LEN).collect()
        } else {
            topic.to_string()
        }
    }

    /// Show the overlay for `topic` (truncated to [`MAX_TOPIC_LEN`]).
    pub fn show(&mut self, topic: &str) {
        self.topic = Self::cap(topic);
        self.visible = true;
    }

    /// Hide the overlay, keeping the last topic.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Hide when already showing `topic`, else show it.
    pub fn toggle(&mut self, topic: &str) {
        let capped = Self::cap(topic);
        if self.visible && self.topic == capped {
            self.hide();
        } else {
            self.topic = capped;
            self.visible = true;
        }
    }

    /// `"help <topic> shown|hidden"`.
    #[must_use]
    pub fn label(&self) -> String {
        let state = if self.visible { "shown" } else { "hidden" };
        format!("help {} {state}", self.topic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_hidden() {
        let ctx = HelperCtx::new();
        assert!(!ctx.visible);
        assert_eq!(ctx.topic, "");
        assert_eq!(ctx.label(), "help  hidden");
    }

    #[test]
    fn show_sets_topic_and_visible() {
        let mut ctx = HelperCtx::new();
        ctx.show("topics");
        assert!(ctx.visible);
        assert_eq!(ctx.topic, "topics");
        assert_eq!(ctx.label(), "help topics shown");
    }

    #[test]
    fn hide_keeps_topic() {
        let mut ctx = HelperCtx::new();
        ctx.show("keys");
        ctx.hide();
        assert!(!ctx.visible);
        assert_eq!(ctx.topic, "keys");
        assert_eq!(ctx.label(), "help keys hidden");
    }

    #[test]
    fn toggle_shows_then_switches() {
        let mut ctx = HelperCtx::new();
        ctx.toggle("a");
        assert!(ctx.visible);
        ctx.toggle("b");
        assert!(ctx.visible);
        assert_eq!(ctx.topic, "b");
        ctx.toggle("b");
        assert!(!ctx.visible);
    }

    #[test]
    fn toggle_hides_same_topic() {
        let mut ctx = HelperCtx::new();
        ctx.show("keys");
        ctx.toggle("keys");
        assert!(!ctx.visible);
    }

    #[test]
    fn truncates_long_topic() {
        let mut ctx = HelperCtx::new();
        ctx.show(&"x".repeat(MAX_TOPIC_LEN + 10));
        assert_eq!(ctx.topic.chars().count(), MAX_TOPIC_LEN);
        assert!(ctx.visible);
    }
}
