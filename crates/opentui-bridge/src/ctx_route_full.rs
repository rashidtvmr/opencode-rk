#![forbid(unsafe_code)]
//! CtxRoute: home/session/plugin navigation state.
//! Mirror of `packages/tui/src/context/route.tsx` Route union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CtxRoute {
    pub kind: String,
    pub id: String,
}
impl CtxRoute {
    pub fn go_home() -> Self {
        Self {
            kind: "home".into(),
            id: String::new(),
        }
    }
    pub fn go_session(id: &str) -> Self {
        Self {
            kind: "session".into(),
            id: id.chars().take(128).collect(),
        }
    }
    pub fn go_plugin(id: &str) -> Self {
        Self {
            kind: "plugin".into(),
            id: id.chars().take(128).collect(),
        }
    }
    pub fn describe(&self) -> String {
        let kind: String = self.kind.chars().take(16).collect();
        let s = if self.id.is_empty() {
            kind
        } else {
            format!("{kind}:{}", self.id)
        };
        s.chars().take(160).collect()
    }
}
impl Default for CtxRoute {
    fn default() -> Self {
        Self::go_home()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn home_empty() {
        let r = CtxRoute::go_home();
        assert_eq!(r.describe(), "home");
    }
    #[test]
    fn session_describes() {
        let r = CtxRoute::go_session("abc");
        assert_eq!(r.describe(), "session:abc");
    }
    #[test]
    fn plugin_describes() {
        let r = CtxRoute::go_plugin("p1");
        assert_eq!(r.describe(), "plugin:p1");
    }
    #[test]
    fn id_caps_128() {
        let r = CtxRoute::go_session(&"x".repeat(200));
        assert_eq!(r.id.chars().count(), 128);
    }
    #[test]
    fn describe_caps_160() {
        let r = CtxRoute::go_plugin(&"y".repeat(200));
        assert!(r.describe().chars().count() <= 160);
    }
}
