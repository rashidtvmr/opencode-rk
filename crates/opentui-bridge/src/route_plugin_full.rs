#![forbid(unsafe_code)]
//! PluginRoute: plugin id nav state.
//! Mirror of `PluginRoute` in `packages/tui/src/context/route.tsx`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginRoute {
    pub id: String,
}
impl PluginRoute {
    pub fn set(&mut self, id: &str) {
        self.id = id.chars().take(128).collect();
    }
    pub fn id_of(&self) -> &str {
        &self.id
    }
    pub fn is_set(&self) -> bool {
        !self.id.is_empty()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_by_default() {
        let r = PluginRoute::default();
        assert!(!r.is_set());
        assert_eq!(r.id_of(), "");
    }
    #[test]
    fn set_roundtrip() {
        let mut r = PluginRoute::default();
        r.set("p1");
        assert!(r.is_set());
        assert_eq!(r.id_of(), "p1");
    }
    #[test]
    fn set_caps_128() {
        let mut r = PluginRoute::default();
        r.set(&"x".repeat(200));
        assert_eq!(r.id_of().chars().count(), 128);
        assert!(r.is_set());
    }
    #[test]
    fn set_clears() {
        let mut r = PluginRoute::default();
        r.set("p1");
        r.set("");
        assert!(!r.is_set());
    }
}
