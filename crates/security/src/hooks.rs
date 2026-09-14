//! Bounded pre/post tool hook bus (max 100 hooks).
#![forbid(unsafe_code)]
use std::fmt;
use thiserror::Error;
pub const MAX_HOOKS: usize = 100;
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub struct HookId(pub u64);
impl fmt::Display for HookId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "hook-{}", self.0)
    }
}
#[derive(Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum HookDecision {
    Allow,
    Deny(String),
    Modify(String),
}
#[derive(Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct ToolInvocation {
    pub name: String,
    pub args: serde_json::Value,
}
impl ToolInvocation {
    #[must_use]
    pub fn new(name: impl Into<String>, args: serde_json::Value) -> Self {
        Self {
            name: name.into(),
            args,
        }
    }
}
#[derive(Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct ToolResult {
    pub name: String,
    pub success: bool,
    pub duration_ms: u64,
}
impl ToolResult {
    #[must_use]
    pub fn new(name: impl Into<String>, success: bool, duration_ms: u64) -> Self {
        Self {
            name: name.into(),
            success,
            duration_ms,
        }
    }
}
pub struct PreHook {
    pub id: HookId,
    pub filter: String,
    pub handler: Box<dyn Fn(&ToolInvocation) -> HookDecision + Send + Sync>,
}
impl PreHook {
    #[must_use]
    pub fn new(
        filter: impl Into<String>,
        handler: impl Fn(&ToolInvocation) -> HookDecision + Send + Sync + 'static,
    ) -> Self {
        Self {
            id: HookId(0),
            filter: filter.into(),
            handler: Box::new(handler),
        }
    }
    #[must_use]
    pub fn matches(&self, inv: &ToolInvocation) -> bool {
        crate::glob_match(&self.filter, &inv.name)
    }
}
pub struct PostHook {
    pub id: HookId,
    pub handler: Box<dyn Fn(&ToolResult) + Send + Sync>,
}
impl PostHook {
    #[must_use]
    pub fn new(handler: impl Fn(&ToolResult) + Send + Sync + 'static) -> Self {
        Self {
            id: HookId(0),
            handler: Box::new(handler),
        }
    }
}
#[derive(Clone, Eq, PartialEq, Debug, Error)]
pub enum HookError {
    #[error("hook bus at capacity ({capacity})")]
    AtCapacity { capacity: usize },
}
pub struct HookBus {
    pre: Vec<PreHook>,
    post: Vec<PostHook>,
    capacity: usize,
    next: u64,
}
impl HookBus {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            pre: Vec::new(),
            post: Vec::new(),
            capacity: capacity.min(MAX_HOOKS),
            next: 1,
        }
    }
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.pre.len() + self.post.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn register_pre(&mut self, mut hook: PreHook) -> Result<HookId, HookError> {
        if self.len() >= self.capacity {
            return Err(HookError::AtCapacity {
                capacity: self.capacity,
            });
        }
        let id = HookId(self.next);
        self.next = self.next.saturating_add(1);
        hook.id = id;
        self.pre.push(hook);
        Ok(id)
    }
    pub fn register_post(&mut self, mut hook: PostHook) -> Result<HookId, HookError> {
        if self.len() >= self.capacity {
            return Err(HookError::AtCapacity {
                capacity: self.capacity,
            });
        }
        let id = HookId(self.next);
        self.next = self.next.saturating_add(1);
        hook.id = id;
        self.post.push(hook);
        Ok(id)
    }
    #[must_use]
    pub fn run_pre_hooks(&self, inv: &ToolInvocation) -> HookDecision {
        let mut modified = None;
        for hook in &self.pre {
            if !hook.matches(inv) {
                continue;
            }
            match (hook.handler)(inv) {
                HookDecision::Deny(reason) => return HookDecision::Deny(reason),
                HookDecision::Modify(patch) => {
                    if modified.is_none() {
                        modified = Some(patch);
                    }
                }
                HookDecision::Allow => {}
            }
        }
        if let Some(patch) = modified {
            HookDecision::Modify(patch)
        } else {
            HookDecision::Allow
        }
    }
    pub fn run_post_hooks(&self, result: &ToolResult) {
        for hook in &self.post {
            (hook.handler)(result);
        }
    }
    #[must_use]
    pub fn unregister(&mut self, id: HookId) -> bool {
        let n = self.len();
        self.pre.retain(|hook| hook.id != id);
        self.post.retain(|hook| hook.id != id);
        self.len() != n
    }
}
impl Default for HookBus {
    fn default() -> Self {
        Self::new(MAX_HOOKS)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    };
    fn inv(name: &str) -> ToolInvocation {
        ToolInvocation::new(name, serde_json::json!({"path":"/work/project/a.rs"}))
    }
    fn res(name: &str) -> ToolResult {
        ToolResult::new(name, true, 12)
    }
    #[test]
    fn pre_hook_can_block() {
        let mut bus = HookBus::default();
        bus.register_pre(PreHook::new("rm", |_| {
            HookDecision::Deny("blocked by policy".to_owned())
        }))
        .unwrap();
        assert!(matches!(
            bus.run_pre_hooks(&inv("rm")),
            HookDecision::Deny(_)
        ));
        assert!(matches!(bus.run_pre_hooks(&inv("ls")), HookDecision::Allow));
    }
    #[test]
    fn post_hook_observes() {
        let mut bus = HookBus::default();
        let seen = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&seen);
        bus.register_post(PostHook::new(move |_| {
            flag.store(true, Ordering::SeqCst);
        }))
        .unwrap();
        assert!(!seen.load(Ordering::SeqCst));
        bus.run_post_hooks(&res("ls"));
        assert!(seen.load(Ordering::SeqCst));
    }
    #[test]
    fn bounded_to_capacity() {
        let mut bus = HookBus::new(100);
        for _ in 0..100 {
            bus.register_pre(PreHook::new("*", |_| HookDecision::Allow))
                .unwrap();
        }
        assert!(matches!(
            bus.register_pre(PreHook::new("*", |_| HookDecision::Allow)),
            Err(HookError::AtCapacity { .. })
        ));
        assert!(matches!(
            bus.register_post(PostHook::new(|_| {})),
            Err(HookError::AtCapacity { .. })
        ));
    }
    #[test]
    fn first_deny_wins() {
        let mut bus = HookBus::default();
        bus.register_pre(PreHook::new("*", |_| HookDecision::Allow))
            .unwrap();
        bus.register_pre(PreHook::new("*", |_| {
            HookDecision::Deny("second says no".to_owned())
        }))
        .unwrap();
        bus.register_pre(PreHook::new("*", |_| HookDecision::Allow))
            .unwrap();
        assert!(
            matches!(bus.run_pre_hooks(&inv("tool")),HookDecision::Deny(reason) if reason=="second says no")
        );
    }
    #[test]
    fn unregister_removes() {
        let mut bus = HookBus::default();
        let calls = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&calls);
        let id = bus
            .register_pre(PreHook::new("*", move |_| {
                count.fetch_add(1, Ordering::SeqCst);
                HookDecision::Allow
            }))
            .unwrap();
        assert!(bus.unregister(id));
        assert!(!bus.unregister(id));
        assert!(matches!(
            bus.run_pre_hooks(&inv("tool")),
            HookDecision::Allow
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}
