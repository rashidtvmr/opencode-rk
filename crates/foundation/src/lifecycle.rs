//! Ordered startup/shutdown lifecycle manager (BASE-007).
use std::sync::Mutex;
use thiserror::Error;
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Phase {
    #[default]
    Init,
    Starting,
    Running,
    Stopping,
    Stopped,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct HookId(pub u64);
pub type HookFn = Box<dyn FnOnce() -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send>;
#[derive(Debug, Error, Eq, PartialEq)]
pub enum LifecycleError {
    #[error("lifecycle already started")]
    AlreadyStarted,
    #[error("lifecycle not running")]
    NotRunning,
    #[error("hook registration closed in phase {0:?}")]
    RegistrationClosed(Phase),
    #[error("lifecycle lock poisoned")]
    LockPoisoned,
    #[error("hook {0:?} failed: {1}")]
    HookFailed(HookId, String),
}
struct RegisteredHook {
    id: HookId,
    priority: u32,
    order: u64,
    hook: Option<HookFn>,
}
struct Inner {
    phase: Phase,
    startup: Vec<RegisteredHook>,
    shutdown: Vec<RegisteredHook>,
    next_id: u64,
    next_order: u64,
}
pub struct LifecycleManager {
    inner: Mutex<Inner>,
}
impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}
impl LifecycleManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                phase: Phase::Init,
                startup: Vec::new(),
                shutdown: Vec::new(),
                next_id: 0,
                next_order: 0,
            }),
        }
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Inner>, LifecycleError> {
        self.inner.lock().map_err(|_| LifecycleError::LockPoisoned)
    }
    #[must_use]
    pub fn phase(&self) -> Phase {
        self.inner.lock().map(|g| g.phase).unwrap_or(Phase::Init)
    }
    pub fn register_startup_hook(
        &self,
        priority: u32,
        hook: HookFn,
    ) -> Result<HookId, LifecycleError> {
        let mut g = self.lock()?;
        if g.phase != Phase::Init {
            return Err(LifecycleError::RegistrationClosed(g.phase));
        }
        let id = HookId(g.next_id);
        g.next_id += 1;
        let order = g.next_order;
        g.next_order += 1;
        g.startup.push(RegisteredHook {
            id,
            priority,
            order,
            hook: Some(hook),
        });
        Ok(id)
    }
    pub fn register_shutdown_hook(
        &self,
        priority: u32,
        hook: HookFn,
    ) -> Result<HookId, LifecycleError> {
        let mut g = self.lock()?;
        if g.phase != Phase::Init {
            return Err(LifecycleError::RegistrationClosed(g.phase));
        }
        let id = HookId(g.next_id);
        g.next_id += 1;
        let order = g.next_order;
        g.next_order += 1;
        g.shutdown.push(RegisteredHook {
            id,
            priority,
            order,
            hook: Some(hook),
        });
        Ok(id)
    }
    pub fn start(&self) -> Result<(), LifecycleError> {
        let mut hooks = {
            let mut g = self.lock()?;
            if g.phase != Phase::Init {
                return Err(LifecycleError::AlreadyStarted);
            }
            g.phase = Phase::Starting;
            let mut h = std::mem::take(&mut g.startup);
            h.sort_by_key(|r| (r.priority, r.order));
            h
        };
        for r in hooks.iter_mut() {
            if let Some(f) = r.hook.take() {
                f().map_err(|e| LifecycleError::HookFailed(r.id, e.to_string()))?;
            }
        }
        self.lock()?.phase = Phase::Running;
        Ok(())
    }
    pub fn stop(&self) -> Result<(), LifecycleError> {
        let mut hooks = {
            let mut g = self.lock()?;
            if g.phase != Phase::Running {
                return Err(LifecycleError::NotRunning);
            }
            g.phase = Phase::Stopping;
            let mut h = std::mem::take(&mut g.shutdown);
            h.sort_by(|a, b| b.priority.cmp(&a.priority).then(b.order.cmp(&a.order)));
            h
        };
        for r in hooks.iter_mut() {
            if let Some(f) = r.hook.take() {
                f().map_err(|e| LifecycleError::HookFailed(r.id, e.to_string()))?;
            }
        }
        self.lock()?.phase = Phase::Stopped;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};
    #[test]
    fn phase_transitions() {
        let m = LifecycleManager::new();
        assert_eq!(m.phase(), Phase::Init);
        m.start().unwrap();
        assert_eq!(m.phase(), Phase::Running);
        m.stop().unwrap();
        assert_eq!(m.phase(), Phase::Stopped);
    }
    #[test]
    fn startup_hooks_ordered() {
        let m = LifecycleManager::new();
        let log = Arc::new(StdMutex::new(Vec::new()));
        for (p, v) in [(10u32, "c"), (1, "a"), (5, "b")] {
            let l = Arc::clone(&log);
            m.register_startup_hook(
                p,
                Box::new(move || {
                    l.lock().unwrap().push(v);
                    Ok(())
                }),
            )
            .unwrap();
        }
        m.start().unwrap();
        assert_eq!(*log.lock().unwrap(), vec!["a", "b", "c"]);
        m.stop().unwrap();
    }
    #[test]
    fn shutdown_hooks_reverse() {
        let m = LifecycleManager::new();
        let log = Arc::new(StdMutex::new(Vec::new()));
        for (p, v) in [(1u32, "a"), (5, "b"), (10, "c")] {
            let l = Arc::clone(&log);
            m.register_shutdown_hook(
                p,
                Box::new(move || {
                    l.lock().unwrap().push(v);
                    Ok(())
                }),
            )
            .unwrap();
        }
        m.start().unwrap();
        m.stop().unwrap();
        assert_eq!(*log.lock().unwrap(), vec!["c", "b", "a"]);
    }
    #[test]
    fn no_hooks_after_running() {
        let m = LifecycleManager::new();
        m.start().unwrap();
        assert!(m.register_startup_hook(1, Box::new(|| Ok(()))).is_err());
        assert!(m.register_shutdown_hook(1, Box::new(|| Ok(()))).is_err());
        m.stop().unwrap();
    }
    #[test]
    fn double_start_fails() {
        let m = LifecycleManager::new();
        m.start().unwrap();
        assert_eq!(m.start(), Err(LifecycleError::AlreadyStarted));
        m.stop().unwrap();
    }
}
