#![forbid(unsafe_code)]
//! Versioned `createSignal<i64>` cell (`util/signal.ts`): value plus monotonic version.

/// Versioned signal cell: `createSignal` value with write counter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SigCell {
    pub v: i64,
    pub ver: u64,
}

impl SigCell {
    /// Write value, bump version.
    pub fn set(&mut self, v: i64) {
        self.v = v;
        self.ver = self.ver.saturating_add(1);
    }
    /// Read value (`Accessor` call).
    #[must_use]
    pub fn get(&self) -> i64 {
        self.v
    }
    /// Write counter.
    #[must_use]
    pub fn ver(&self) -> u64 {
        self.ver
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        let c = SigCell::default();
        assert_eq!(c.get(), 0);
        assert_eq!(c.ver(), 0);
    }

    #[test]
    fn set_bumps_version() {
        let mut c = SigCell::default();
        c.set(41);
        c.set(42);
        assert_eq!(c.get(), 42);
        assert_eq!(c.ver(), 2);
    }

    #[test]
    fn same_value_still_bumps() {
        let mut c = SigCell { v: 7, ver: 0 };
        c.set(7);
        assert_eq!(c.get(), 7);
        assert_eq!(c.ver(), 1);
    }
}
