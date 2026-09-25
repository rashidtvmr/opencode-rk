#![forbid(unsafe_code)]
//! Online connection flag with flip counter.
//!
//! Port of `useConnected` (packages/tui/src/component/use-connected.tsx:4-12):
//! TS memo is true when any provider is connected (id != "opencode" or a
//! model with nonzero input cost). `Connected` stores that bool; `flips`
//! counts false<->true transitions for banner/debounce use.

/// Connection state + transition count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Connected {
    online: bool,
    flips: u32,
}

impl Connected {
    /// New state with zero flips.
    #[must_use]
    pub fn new(online: bool) -> Self {
        Self { online, flips: 0 }
    }

    /// Set online; bump `flips` only on change.
    pub fn set(&mut self, online: bool) {
        if self.online != online {
            self.online = online;
            self.flips = self.flips.saturating_add(1);
        }
    }

    /// Current online flag.
    #[must_use]
    pub fn is_online(&self) -> bool {
        self.online
    }

    /// Number of transitions so far.
    #[must_use]
    pub fn flips(&self) -> u32 {
        self.flips
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_zero_flips() {
        assert_eq!(
            Connected::new(true),
            Connected {
                online: true,
                flips: 0
            }
        );
    }

    #[test]
    fn set_bumps_flips_on_change_only() {
        let mut c = Connected::new(false);
        c.set(false);
        assert_eq!(c.flips(), 0);
        c.set(true);
        assert!(c.is_online());
        assert_eq!(c.flips(), 1);
        c.set(true);
        assert_eq!(c.flips(), 1);
    }

    #[test]
    fn flips_count_each_transition() {
        let mut c = Connected::default();
        c.set(true);
        c.set(false);
        c.set(true);
        assert_eq!(c.flips(), 3);
        assert!(c.is_online());
    }
}
