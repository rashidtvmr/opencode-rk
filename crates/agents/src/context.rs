//! Execution context with variable scoping, secret management, and resource limits.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Resource limits for an execution context.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextLimits {
    /// Maximum tokens allowed for this execution.
    pub max_tokens: Option<u64>,
    /// Maximum duration in seconds.
    pub max_duration_secs: Option<u64>,
    /// Maximum tool calls allowed.
    pub max_tool_calls: Option<u64>,
}

impl ContextLimits {
    /// Create new limits with all values unset.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Execution context for an agent run.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Mutable key-value variables.
    pub variables: HashMap<String, String>,
    /// Secret names that should not be logged.
    pub secrets: HashSet<String>,
    /// Resource limits for this execution.
    pub limits: ContextLimits,
    /// Additional metadata.
    pub metadata: HashMap<String, String>,
}

impl ExecutionContext {
    /// Create a new context with given limits.
    pub fn with_limits(limits: ContextLimits) -> Self {
        Self {
            variables: HashMap::new(),
            secrets: HashSet::new(),
            limits,
            metadata: HashMap::new(),
        }
    }
}

/// Manager for execution contexts with push/pop scoping.
#[derive(Debug, Default)]
pub struct ContextManager {
    /// Stack of variable maps for scoping.
    variables: Vec<HashMap<String, String>>,
    /// Stack of secret sets.
    secrets: Vec<HashSet<String>>,
    /// Stack of limit configs.
    limits: Vec<ContextLimits>,
    /// Global metadata map.
    metadata: HashMap<String, String>,
}

impl ContextManager {
    /// Create a new empty manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a new scope onto the stack.
    pub fn push_scope(&mut self) {
        self.variables.push(HashMap::new());
        self.secrets.push(HashSet::new());
        self.limits.push(ContextLimits::new());
    }

    /// Pop a scope from the stack.
    pub fn pop_scope(&mut self) {
        self.variables.pop();
        self.secrets.pop();
        self.limits.pop();
    }

    /// Set a variable in the top scope.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        if let Some(scope) = self.variables.last_mut() {
            scope.insert(key.into(), value.into());
        }
    }

    /// Get a variable, searching from innermost to outermost scope.
    pub fn get(&self, key: &str) -> Option<&str> {
        if key.starts_with(|c: char| c.is_ascii_uppercase()) {
            return None; // Secret names don't leak through get()
        }
        for scope in self.variables.iter().rev() {
            if let Some(v) = scope.get(key) {
                return Some(v);
            }
        }
        None
    }

    /// Add a secret name to the current scope.
    pub fn add_secret(&mut self, key: impl Into<String>) {
        if let Some(scope) = self.secrets.last_mut() {
            scope.insert(key.into());
        }
    }

    /// Check if a key is a secret name in any scope.
    pub fn has_secret(&self, key: &str) -> bool {
        self.secrets.iter().any(|s| s.contains(key))
    }

    /// Get a plain value (secrets allowed).
    pub fn get_plain(&self, key: &str) -> Option<&str> {
        for scope in self.variables.iter().rev() {
            if let Some(v) = scope.get(key) {
                return Some(v);
            }
        }
        None
    }

    /// Return the current scope depth.
    pub fn depth(&self) -> usize {
        self.variables.len()
    }

    /// Get the current limits.
    pub fn limits(&self) -> ContextLimits {
        self.limits.last().cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get() {
        let mut cm = ContextManager::new();
        cm.set("foo", "bar");
        assert_eq!(cm.get("foo"), Some("bar"));
    }

    #[test]
    fn push_pop_scope() {
        let mut cm = ContextManager::new();
        cm.set("x", "original");
        cm.push_scope();
        cm.set("x", "shadowed");
        assert_eq!(cm.get("x"), Some("shadowed"));
        cm.pop_scope();
        assert_eq!(cm.get("x"), Some("original"));
    }

    #[test]
    fn secret_managed() {
        let mut cm = ContextManager::new();
        assert!(!cm.has_secret("SECRET"));
        cm.push_scope();
        cm.add_secret("SECRET");
        assert!(cm.has_secret("SECRET"));
        // But secret names don't leak via get()
        assert!(cm.get("SECRET").is_none());
    }

    #[test]
    fn limit_defined() {
        let limits = ContextLimits {
            max_tokens: Some(1000),
            max_duration_secs: Some(30),
            max_tool_calls: Some(5),
        };
        cm.push_scope();
        cm.limits_mut().max_tokens = Some(1000);
        cm.limits_mut().max_duration_secs = Some(30);
        cm.limits_mut().max_tool_calls = Some(5);
        assert_eq!(cm.limits().max_tokens, Some(1000));
        assert_eq!(cm.limits().max_duration_secs, Some(30));
        assert_eq!(cm.limits().max_tool_calls, Some(5));
        cm.pop_scope();
        assert_eq!(cm.limits().max_tokens, None);
    }

    #[test]
    fn scoped_variables() {
        let mut cm = ContextManager::new();
        cm.set("global", "g");
        assert_eq!(cm.depth(), 1);
        cm.push_scope();
        cm.set("local", "l");
        assert_eq!(cm.get("global"), Some("g"));
        assert_eq!(cm.get("local"), Some("l"));
        cm.pop_scope();
        assert_eq!(cm.depth(), 1);
    }
}