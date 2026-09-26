#![forbid(unsafe_code)]
//! Thinking level mirror of TS thinking context.
//!
//! TS: packages/tui/src/context/thinking.ts:4 (show|hide), :24-27
//! (show->hide->show), :29-67 (kv.signal thinking_mode default hide +
//! legacy thinking_visibility bool migrate, :56 minimal->hide).
//! Divergence: u8 level 0=off(hide) 1=low 2=high; `migrated` mirrors the
//! legacy-bool migration branch. reasoningSummary skipped (ponytail: add
//! when bridge parses markdown).

/// Thinking context: level 0..=2, migrated tracks legacy migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThinkingCtx {
    pub level: u8,
    pub migrated: bool,
}

impl Default for ThinkingCtx {
    /// TS :36 default "hide" -> off.
    fn default() -> Self {
        Self::new()
    }
}

impl ThinkingCtx {
    /// Off, unmigrated (first-time user, TS :50-51 no legacy key).
    #[must_use]
    pub fn new() -> Self {
        Self {
            level: 0,
            migrated: false,
        }
    }

    /// Set level; returns false and keeps old on >2.
    pub fn set_level(&mut self, level: u8) -> bool {
        if level > 2 {
            return false;
        }
        self.level = level;
        true
    }

    /// Mark legacy migration done (TS :51-54).
    pub fn mark_migrated(&mut self) {
        self.migrated = true;
    }

    /// Label for level: off|low|high.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self.level {
            0 => "off",
            1 => "low",
            _ => "high",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_off_unmigrated() {
        let ctx = ThinkingCtx::new();
        assert_eq!(ctx.level, 0);
        assert_eq!(ctx.label(), "off");
        assert!(!ctx.migrated);
        assert_eq!(ThinkingCtx::default(), ctx);
    }

    #[test]
    fn set_level_accepts_bounds() {
        let mut ctx = ThinkingCtx::new();
        assert!(ctx.set_level(1));
        assert_eq!(ctx.level, 1);
        assert!(ctx.set_level(2));
        assert_eq!(ctx.level, 2);
        assert!(ctx.set_level(0));
        assert_eq!(ctx.level, 0);
    }

    #[test]
    fn set_level_rejects_above_two() {
        let mut ctx = ThinkingCtx::new();
        ctx.set_level(2);
        assert!(!ctx.set_level(3));
        assert!(!ctx.set_level(u8::MAX));
        assert_eq!(ctx.level, 2);
    }

    #[test]
    fn labels_cover_all_levels() {
        let mut ctx = ThinkingCtx::new();
        assert_eq!(ctx.label(), "off");
        ctx.set_level(1);
        assert_eq!(ctx.label(), "low");
        ctx.set_level(2);
        assert_eq!(ctx.label(), "high");
    }

    #[test]
    fn mark_migrated_sets_flag() {
        let mut ctx = ThinkingCtx::new();
        assert!(!ctx.migrated);
        ctx.mark_migrated();
        assert!(ctx.migrated);
    }

    #[test]
    fn migrated_flag_survives_level_changes() {
        let mut ctx = ThinkingCtx::new();
        ctx.mark_migrated();
        ctx.set_level(2);
        assert!(ctx.migrated);
        assert_eq!(ctx.label(), "high");
    }
}
