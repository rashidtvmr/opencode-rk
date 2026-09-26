#![forbid(unsafe_code)]
//! Shared runtime handle table.
//!
//! Mirrors `packages/opencode/src/cli/cmd/run/runtime.shared.ts`
//! (`reusePendingTask` slot keyed by runtime id) plus the lifecycle
//! in `crate::run_runtime`: ids capped at 64 chars, table capped at 16.

/// Max chars per runtime id.
pub const RUNTIME_ID_CAP: usize = 64;
/// Max handles in the table.
pub const RUNTIME_TABLE_CAP: usize = 16;

/// One tracked runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeHandle {
    id: String,
    alive: bool,
}

impl RuntimeHandle {
    /// Handle id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Whether the runtime is alive.
    pub fn is_alive(&self) -> bool {
        self.alive
    }
}

/// Bounded table of runtime handles.
#[derive(Debug, Default)]
pub struct RuntimeTable {
    handles: Vec<RuntimeHandle>,
}

impl RuntimeTable {
    /// Empty table.
    pub fn new() -> Self {
        Self {
            handles: Vec::new(),
        }
    }

    /// Handle count.
    pub fn len(&self) -> usize {
        self.handles.len()
    }

    /// True when no handles registered.
    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }

    /// Register an id; errs on empty/dup/cap.
    pub fn register(&mut self, id: &str) -> Result<(), String> {
        if id.is_empty() {
            return Err("empty id".to_string());
        }
        if id.len() > RUNTIME_ID_CAP {
            return Err("id too long".to_string());
        }
        if self.handles.iter().any(|h| h.id == id) {
            return Err("duplicate id".to_string());
        }
        if self.handles.len() >= RUNTIME_TABLE_CAP {
            return Err("table full".to_string());
        }
        self.handles.push(RuntimeHandle {
            id: id.to_string(),
            alive: true,
        });
        Ok(())
    }

    /// Mark dead; false when unknown.
    pub fn kill(&mut self, id: &str) -> bool {
        match self.handles.iter_mut().find(|h| h.id == id) {
            Some(h) => {
                h.alive = false;
                true
            }
            None => false,
        }
    }

    /// True only for known alive handles.
    pub fn is_alive(&self, id: &str) -> bool {
        self.handles.iter().any(|h| h.id == id && h.alive)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_ok() {
        let mut t = RuntimeTable::new();
        assert!(t.register("r1").is_ok());
        assert_eq!(t.len(), 1);
        assert!(t.is_alive("r1"));
    }

    #[test]
    fn register_dup_errs() {
        let mut t = RuntimeTable::new();
        t.register("r1").unwrap();
        assert_eq!(t.register("r1"), Err("duplicate id".to_string()));
    }

    #[test]
    fn register_empty_errs() {
        let mut t = RuntimeTable::new();
        assert_eq!(t.register(""), Err("empty id".to_string()));
    }

    #[test]
    fn register_cap_errs() {
        let mut t = RuntimeTable::new();
        for i in 0..RUNTIME_TABLE_CAP {
            t.register(&format!("r{i}")).unwrap();
        }
        assert_eq!(t.register("overflow"), Err("table full".to_string()));
        let long = "x".repeat(RUNTIME_ID_CAP + 1);
        let mut u = RuntimeTable::new();
        assert_eq!(u.register(&long), Err("id too long".to_string()));
    }

    #[test]
    fn kill_flips_alive() {
        let mut t = RuntimeTable::new();
        t.register("r1").unwrap();
        assert!(t.kill("r1"));
        assert!(!t.is_alive("r1"));
        assert!(!t.kill("missing"));
    }

    #[test]
    fn alive_false_unknown() {
        let t = RuntimeTable::new();
        assert!(!t.is_alive("nope"));
        assert!(t.is_empty());
    }
}
