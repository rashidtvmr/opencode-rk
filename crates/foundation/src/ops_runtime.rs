//! Effect-runtime tick helpers (OPS-007).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTick {
    pub n: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeError2 {
    Overflow,
}

impl std::fmt::Display for RuntimeError2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Overflow => write!(f, "tick counter overflow"),
        }
    }
}

impl std::error::Error for RuntimeError2 {}

pub fn next_tick(t: &RuntimeTick) -> Result<RuntimeTick, RuntimeError2> {
    t.n.checked_add(1)
        .map(|n| RuntimeTick { n })
        .ok_or(RuntimeError2::Overflow)
}

#[must_use]
pub fn ticks_since(a: u64, b: u64) -> u64 {
    b.saturating_sub(a)
}
