#![forbid(unsafe_code)]
//! Layered theme registry: defaults < plugin < custom < system.
//!
//! TS truth: `packages/tui/src/theme/index.ts:171-239` (`listThemes`,
//! `subscribeThemes`/`syncThemes` fanout, `addTheme` reject-dup, `isTheme`
//! guard, `upsertTheme` custom-hit-else-plugin split). Divergence: sync
//! listener fanout (no Solid signals); bounded caps.

/// Listener cap (TS set is unbounded).
pub const MAX_LISTENERS: usize = 32;
/// Theme entry cap per layer.
pub const MAX_THEMES: usize = 256;

/// Fail-closed registry error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerError {
    Duplicate,
    Full,
    ListenersFull,
}

/// One named theme entry (name + opaque spec payload).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub spec: String,
}

/// Layered registry with sync listeners.
#[derive(Default)]
pub struct Layered {
    defaults: Vec<Entry>,
    plugin: Vec<Entry>,
    custom: Vec<Entry>,
    system: Vec<Entry>,
    listeners: Vec<Box<dyn Fn(&str)>>,
}

impl Layered {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn len_total(&self) -> usize {
        self.defaults.len() + self.plugin.len() + self.custom.len() + self.system.len()
    }

    /// Priority lookup: system > custom > plugin > defaults.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.system
            .iter()
            .find(|e| e.name == name)
            .or_else(|| self.custom.iter().find(|e| e.name == name))
            .or_else(|| self.plugin.iter().find(|e| e.name == name))
            .or_else(|| self.defaults.iter().find(|e| e.name == name))
    }

    /// `isTheme` guard.
    #[must_use]
    pub fn is_theme(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    fn has_any(&self, name: &str) -> bool {
        self.is_theme(name)
    }

    /// Seed a defaults entry (dup rejected).
    pub fn add_default(&mut self, name: &str, spec: &str) -> Result<(), LayerError> {
        if self.has_any(name) {
            return Err(LayerError::Duplicate);
        }
        if self.len_total() >= MAX_THEMES {
            return Err(LayerError::Full);
        }
        self.defaults.push(Entry {
            name: String::from(name),
            spec: String::from(spec),
        });
        Ok(())
    }

    /// `addTheme` reject-dup into plugin layer.
    pub fn add_theme(&mut self, name: &str, spec: &str) -> Result<(), LayerError> {
        if self.has_any(name) {
            return Err(LayerError::Duplicate);
        }
        if self.len_total() >= MAX_THEMES {
            return Err(LayerError::Full);
        }
        self.plugin.push(Entry {
            name: String::from(name),
            spec: String::from(spec),
        });
        self.notify(name);
        Ok(())
    }

    /// `upsertTheme`: custom hit updates custom, else plugin.
    pub fn upsert(&mut self, name: &str, spec: &str) -> Result<(), LayerError> {
        if let Some(e) = self.custom.iter_mut().find(|e| e.name == name) {
            e.spec = String::from(spec);
            self.notify(name);
            return Ok(());
        }
        if let Some(e) = self.plugin.iter_mut().find(|e| e.name == name) {
            e.spec = String::from(spec);
            self.notify(name);
            return Ok(());
        }
        if self.len_total() >= MAX_THEMES {
            return Err(LayerError::Full);
        }
        self.plugin.push(Entry {
            name: String::from(name),
            spec: String::from(spec),
        });
        self.notify(name);
        Ok(())
    }

    /// `subscribeThemes` listener; returns listener count.
    pub fn subscribe(&mut self, f: Box<dyn Fn(&str)>) -> Result<usize, LayerError> {
        if self.listeners.len() >= MAX_LISTENERS {
            return Err(LayerError::ListenersFull);
        }
        self.listeners.push(f);
        Ok(self.listeners.len())
    }

    fn notify(&self, name: &str) {
        for l in &self.listeners {
            l(name);
        }
    }

    #[must_use]
    pub fn listener_count(&self) -> usize {
        self.listeners.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    use core::cell::RefCell;

    #[test]
    fn priority_order() {
        let mut l = Layered::new();
        l.add_default("t", "d").unwrap();
        l.add_theme("t2", "p").unwrap();
        l.upsert("t", "c").unwrap();
        assert_eq!(l.get("t").unwrap().spec, "c");
        assert_eq!(l.get("t2").unwrap().spec, "p");
    }

    #[test]
    fn upsert_split() {
        let mut l = Layered::new();
        l.upsert("n", "p1").unwrap();
        assert_eq!(l.get("n").unwrap().spec, "p1");
        l.upsert("n", "p2").unwrap();
        assert_eq!(l.get("n").unwrap().spec, "p2");
    }

    #[test]
    fn dup_rejected() {
        let mut l = Layered::new();
        l.add_default("t", "d").unwrap();
        assert_eq!(l.add_theme("t", "x"), Err(LayerError::Duplicate));
    }

    #[test]
    fn listener_notified() {
        let mut l = Layered::new();
        let seen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let c = seen.clone();
        l.subscribe(Box::new(move |n| c.borrow_mut().push(String::from(n))))
            .unwrap();
        l.add_theme("t", "p").unwrap();
        assert_eq!(seen.borrow().len(), 1);
        assert_eq!(l.listener_count(), 1);
    }

    #[test]
    fn listeners_full() {
        let mut l = Layered::new();
        for _ in 0..MAX_LISTENERS {
            l.subscribe(Box::new(|_| {})).unwrap();
        }
        assert_eq!(
            l.subscribe(Box::new(|_| {})),
            Err(LayerError::ListenersFull)
        );
    }
}
