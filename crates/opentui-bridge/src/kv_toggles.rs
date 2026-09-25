#![forbid(unsafe_code)]

//! KV-backed UI toggles (diff wrap, assistant metadata, animations).

/// UI toggle bits stored alongside [`crate::context_kv::KvStore`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KvToggles {
    pub diff_wrap_mode: bool,
    pub assistant_metadata: bool,
    pub animations_enabled: bool,
}

/// Selectable toggle bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggle {
    DiffWrap,
    AssistantMeta,
    Animations,
}

impl KvToggles {
    /// All-false defaults.
    #[must_use]
    pub fn defaults() -> Self {
        Self::default()
    }

    /// Flip the selected bit, return its new value.
    pub fn apply(&mut self, toggle: Toggle) -> bool {
        match toggle {
            Toggle::DiffWrap => {
                self.diff_wrap_mode = !self.diff_wrap_mode;
                self.diff_wrap_mode
            }
            Toggle::AssistantMeta => {
                self.assistant_metadata = !self.assistant_metadata;
                self.assistant_metadata
            }
            Toggle::Animations => {
                self.animations_enabled = !self.animations_enabled;
                self.animations_enabled
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_all_false() {
        let t = KvToggles::defaults();
        assert!(!t.diff_wrap_mode && !t.assistant_metadata && !t.animations_enabled);
    }

    #[test]
    fn diff_wrap_flips() {
        let mut t = KvToggles::defaults();
        assert!(t.apply(Toggle::DiffWrap));
        assert!(t.diff_wrap_mode);
    }

    #[test]
    fn assistant_meta_flips() {
        let mut t = KvToggles::defaults();
        assert!(t.apply(Toggle::AssistantMeta));
        assert!(t.assistant_metadata);
    }

    #[test]
    fn animations_flips() {
        let mut t = KvToggles::defaults();
        assert!(t.apply(Toggle::Animations));
        assert!(t.animations_enabled);
    }

    #[test]
    fn double_flip_restores() {
        let mut t = KvToggles::defaults();
        t.apply(Toggle::DiffWrap);
        assert!(!t.apply(Toggle::DiffWrap));
        assert_eq!(t, KvToggles::defaults());
    }

    #[test]
    fn bits_independent() {
        let mut t = KvToggles::defaults();
        t.apply(Toggle::DiffWrap);
        assert!(t.diff_wrap_mode && !t.assistant_metadata && !t.animations_enabled);
        t.apply(Toggle::Animations);
        assert!(t.diff_wrap_mode && !t.assistant_metadata && t.animations_enabled);
    }
}
