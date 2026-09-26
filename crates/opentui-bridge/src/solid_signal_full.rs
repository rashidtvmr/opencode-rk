#![forbid(unsafe_code)]
//! Owned string signal port of `createSignal`/`createMemo` in signal.ts.
//! ponytail: no subscribers/effects graph, add when signal_graph wires it.

/// Max chars stored per value / memo output.
pub const MAX_CHARS: usize = 4096;

/// Owned string cell with monotonically bumped version.
#[derive(Debug, Clone, Default)]
pub struct SolidSignal {
    value: String,
    version: u64,
}

fn cap(s: &str) -> String {
    s.chars().take(MAX_CHARS).collect()
}

impl SolidSignal {
    /// New signal, capped, version 0.
    #[must_use]
    pub fn new(s: &str) -> Self {
        Self {
            value: cap(s),
            version: 0,
        }
    }
    /// Current value.
    #[must_use]
    pub fn get(&self) -> &str {
        &self.value
    }
    /// Version bumped on every set (wrapping).
    #[must_use]
    pub fn version(&self) -> u64 {
        self.version
    }
    /// Replace value (capped), bump version.
    pub fn set(&mut self, s: &str) {
        self.value = cap(s);
        self.version = self.version.wrapping_add(1);
    }
    /// Derived value: `f(get())`, capped to 4KiB chars.
    #[must_use]
    pub fn memo(&self, f: &dyn Fn(&str) -> String) -> String {
        cap(&f(&self.value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_caps_and_v0() {
        let s = SolidSignal::new(&"x".repeat(MAX_CHARS + 10));
        assert_eq!(s.get().chars().count(), MAX_CHARS);
        assert_eq!(s.version(), 0);
    }

    #[test]
    fn set_bumps_version() {
        let mut s = SolidSignal::new("a");
        s.set("b");
        assert_eq!(s.get(), "b");
        assert_eq!(s.version(), 1);
        s.set("c");
        assert_eq!(s.version(), 2);
    }

    #[test]
    fn set_caps_long() {
        let mut s = SolidSignal::new("");
        s.set(&"y".repeat(MAX_CHARS + 5));
        assert_eq!(s.get().chars().count(), MAX_CHARS);
    }

    #[test]
    fn memo_derives_capped() {
        let s = SolidSignal::new("hi");
        assert_eq!(s.memo(&|v| format!("{v}!")), "hi!");
        let big = s.memo(&|_| "z".repeat(MAX_CHARS + 1));
        assert_eq!(big.chars().count(), MAX_CHARS);
    }

    #[test]
    fn version_wraps() {
        let mut s = SolidSignal {
            value: String::new(),
            version: u64::MAX,
        };
        s.set("w");
        assert_eq!(s.version(), 0);
        assert_eq!(s.get(), "w");
    }

    #[test]
    fn default_empty() {
        let s = SolidSignal::default();
        assert_eq!(s.get(), "");
        assert_eq!(s.version(), 0);
    }
}
