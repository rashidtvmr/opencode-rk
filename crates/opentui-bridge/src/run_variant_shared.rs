#![forbid(unsafe_code)]
//! Variant pick table: TS truth variant.shared.ts (flag > saved > session;
//! cycleVariant/fitVariant). Bounded rows, cursor, per-row rename. No persist.

/// Max agent label chars.
pub const MAX_AGENT_CHARS: usize = 64;
/// Max model id chars.
pub const MAX_MODEL_CHARS: usize = 128;
/// Max table rows.
pub const MAX_VARIANTS: usize = 32;

fn truncate(s: &str, cap: usize) -> String {
    if s.chars().count() <= cap {
        s.to_owned()
    } else {
        s.chars().take(cap).collect()
    }
}

/// One variant row: agent label + model id, both truncated at construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelVariant {
    /// Agent/provider label, capped at [`MAX_AGENT_CHARS`] chars.
    pub agent: String,
    /// Model id, capped at [`MAX_MODEL_CHARS`] chars.
    pub model: String,
}

impl ModelVariant {
    /// Build a row, truncating over-long fields to their caps.
    #[must_use]
    pub fn new(agent: &str, model: &str) -> Self {
        Self {
            agent: truncate(agent, MAX_AGENT_CHARS),
            model: truncate(model, MAX_MODEL_CHARS),
        }
    }
}

/// Bounded table with a cursor into `items`.
#[derive(Debug, Clone, Default)]
pub struct VariantTable {
    items: Vec<ModelVariant>,
    current: usize,
}

impl VariantTable {
    /// Empty table; cursor 0 (points nowhere until a row is pushed).
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            current: 0,
        }
    }

    /// Push a row; false + no-op when full (`items.len() == 32`).
    pub fn push(&mut self, item: ModelVariant) -> bool {
        if self.items.len() >= MAX_VARIANTS {
            return false;
        }
        self.items.push(item);
        true
    }

    /// Row count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True when no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Move cursor; OOB index returns false and keeps the cursor.
    pub fn select(&mut self, idx: usize) -> bool {
        if idx >= self.items.len() {
            return false;
        }
        self.current = idx;
        true
    }

    /// Current row, or `None` when empty.
    #[must_use]
    pub fn current(&self) -> Option<&ModelVariant> {
        self.items.get(self.current)
    }

    /// Rename a row's model (truncated); OOB index returns false.
    pub fn set_model(&mut self, idx: usize, model: &str) -> bool {
        match self.items.get_mut(idx) {
            None => false,
            Some(row) => {
                row.model = truncate(model, MAX_MODEL_CHARS);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table2() -> VariantTable {
        let mut t = VariantTable::new();
        assert!(t.push(ModelVariant::new("a1", "m1")));
        assert!(t.push(ModelVariant::new("a2", "m2")));
        t
    }

    #[test]
    fn select_ok_moves_cursor() {
        let mut t = table2();
        assert!(t.select(1));
        assert_eq!(t.current().unwrap().model, "m2");
    }

    #[test]
    fn select_oob_false_keeps_cursor() {
        let mut t = table2();
        assert!(t.select(0));
        assert!(!t.select(9));
        assert_eq!(t.current().unwrap().model, "m1");
    }

    #[test]
    fn current_none_when_empty() {
        let t = VariantTable::new();
        assert!(t.is_empty());
        assert!(t.current().is_none());
    }

    #[test]
    fn set_model_ok_truncates() {
        let mut t = table2();
        assert!(t.set_model(0, "m9"));
        assert_eq!(t.current().unwrap().model, "m9");
        assert!(!t.set_model(7, "x"));
    }

    #[test]
    fn caps_truncate_fields_and_rows() {
        let v = ModelVariant::new(&"a".repeat(70), &"m".repeat(140));
        assert_eq!(v.agent.chars().count(), MAX_AGENT_CHARS);
        assert_eq!(v.model.chars().count(), MAX_MODEL_CHARS);
        let mut t = VariantTable::new();
        for i in 0..MAX_VARIANTS {
            assert!(t.push(ModelVariant::new("a", &format!("m{i}"))));
        }
        assert!(!t.push(ModelVariant::new("a", "overflow")));
        assert_eq!(t.len(), MAX_VARIANTS);
    }
}
