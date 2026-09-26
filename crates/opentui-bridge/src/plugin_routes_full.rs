#![forbid(unsafe_code)]
//! Plugin mount flow with route count (PAR-261).

use crate::plugin_adapter_full::PluginAdapter;

/// Flow pairing adapter mounts with successful-mount route count.
#[derive(Debug, Default)]
pub struct PluginFlow {
    pub ad: PluginAdapter,
    pub routes: u32,
}

impl PluginFlow {
    #[must_use]
    pub fn new() -> Self {
        Self {
            ad: PluginAdapter::new(),
            routes: 0,
        }
    }

    pub fn mount(&mut self, id: &str) -> bool {
        if self.ad.mount(id) {
            self.routes = self.routes.saturating_add(1);
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn routes(&self) -> u32 {
        self.routes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_zero() {
        let f = PluginFlow::new();
        assert_eq!(f.routes(), 0);
        assert!(f.ad.mounted_list().is_empty());
    }

    #[test]
    fn mount_bumps() {
        let mut f = PluginFlow::new();
        assert!(f.mount("p-a"));
        assert_eq!(f.routes(), 1);
    }

    #[test]
    fn mount_fail_no_bump() {
        let mut f = PluginFlow::new();
        assert!(!f.mount(""));
        assert_eq!(f.routes(), 0);
        assert!(f.mount("p"));
        assert!(!f.mount("p"));
        assert_eq!(f.routes(), 1);
    }

    #[test]
    fn multi_count() {
        let mut f = PluginFlow::new();
        assert!(f.mount("a"));
        assert!(f.mount("b"));
        assert_eq!(f.routes(), 2);
        assert_eq!(f.ad.mounted_list().len(), 2);
    }

    #[test]
    fn default_zero() {
        let f = PluginFlow::default();
        assert_eq!(f.routes(), 0);
    }
}
