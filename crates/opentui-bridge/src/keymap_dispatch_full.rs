#![forbid(unsafe_code)]
//! Hit/miss-counting dispatch wrapper (BRIDGE-PAR-265).
//!
//! TS truth: `crate::keymap_dispatch::KeymapDispatch`.

use crate::keymap_dispatch::KeymapDispatch;

/// Dispatch flow: inner table plus hit/miss counters.
#[derive(Debug, Default)]
pub struct DispatchFlow {
    pub inner: KeymapDispatch,
    pub hits: u64,
    pub misses: u64,
}

impl DispatchFlow {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_inner(inner: KeymapDispatch) -> Self {
        Self {
            inner,
            hits: 0,
            misses: 0,
        }
    }

    pub fn dispatch(&mut self, key: &str) -> Option<String> {
        match self.inner.dispatch(key) {
            Some(a) => {
                self.hits = self.hits.saturating_add(1);
                Some(a.command.clone())
            }
            None => {
                self.misses = self.misses.saturating_add(1);
                None
            }
        }
    }

    #[must_use]
    pub fn hits(&self) -> u64 {
        self.hits
    }

    #[must_use]
    pub fn misses(&self) -> u64 {
        self.misses
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap_dispatch::KeyAction;

    fn seeded() -> DispatchFlow {
        let mut inner = KeymapDispatch::new();
        inner.bind("ctrl-p", KeyAction::new("Palette", "command.palette.show"));
        DispatchFlow::with_inner(inner)
    }

    #[test]
    fn hit_returns_command_and_counts() {
        let mut f = seeded();
        assert_eq!(
            f.dispatch("ctrl-p").as_deref(),
            Some("command.palette.show")
        );
        assert_eq!(f.hits(), 1);
        assert_eq!(f.misses(), 0);
    }

    #[test]
    fn miss_returns_none_and_counts() {
        let mut f = seeded();
        assert!(f.dispatch("ctrl-z").is_none());
        assert_eq!(f.hits(), 0);
        assert_eq!(f.misses(), 1);
    }

    #[test]
    fn mixed_counts_accumulate() {
        let mut f = seeded();
        f.dispatch("ctrl-p");
        f.dispatch("ctrl-z");
        f.dispatch("ctrl-p");
        assert_eq!(f.hits(), 2);
        assert_eq!(f.misses(), 1);
    }

    #[test]
    fn default_starts_zero() {
        let f = DispatchFlow::new();
        assert_eq!(f.hits(), 0);
        assert_eq!(f.misses(), 0);
    }

    #[test]
    fn unbound_after_unbind_is_miss() {
        let mut f = seeded();
        f.inner.unbind("ctrl-p");
        assert!(f.dispatch("ctrl-p").is_none());
        assert_eq!(f.misses(), 1);
    }
}
