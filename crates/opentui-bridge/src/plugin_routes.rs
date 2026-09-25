#![forbid(unsafe_code)]
//! Plugin route state (mirrors `packages/tui/src/plugin/adapters.tsx:41-80`).
//!
//! Evidence: `adapters.tsx:41-55` `routeNavigate` (home/session/plugin),
//! `:57-73` `routeCurrent`, `:75-80` `mapOption` passthrough.

/// Max route id chars (session id / plugin id, fail-closed truncate).
pub const MAX_ROUTE_ID: usize = 64;

/// Max option label chars (fail-closed truncate).
pub const MAX_OPTION_LABEL: usize = 128;

/// Plugin route: home default, session(id), or plugin(id).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PluginRoute {
    #[default]
    Home,
    Session(String),
    Plugin(String),
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    value.chars().take(max).collect()
}

/// Navigate; mirrors `routeNavigate`. Returns false when session id missing.
pub fn navigate(current: &mut PluginRoute, name: &str, session_id: Option<&str>) -> bool {
    if name == "home" {
        *current = PluginRoute::Home;
        return true;
    }
    if name == "session" {
        let Some(id) = session_id else { return false };
        if id.is_empty() {
            return false;
        }
        *current = PluginRoute::Session(truncate(id, MAX_ROUTE_ID));
        return true;
    }
    *current = PluginRoute::Plugin(truncate(name, MAX_ROUTE_ID));
    true
}

/// Route discriminant name; plugin ids are dynamic so fold to `"plugin"`.
#[must_use]
pub fn current_name(route: &PluginRoute) -> &'static str {
    match route {
        PluginRoute::Home => "home",
        PluginRoute::Session(_) => "session",
        PluginRoute::Plugin(_) => "plugin",
    }
}

/// Option label passthrough with 128-char cap (mirrors `mapOption` spread).
#[must_use]
pub fn map_option_label(label: &str) -> String {
    truncate(label, MAX_OPTION_LABEL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_nav() {
        let mut r = PluginRoute::Plugin("x".to_string());
        assert!(navigate(&mut r, "home", None));
        assert_eq!(r, PluginRoute::Home);
    }

    #[test]
    fn session_nav_ok() {
        let mut r = PluginRoute::Home;
        assert!(navigate(&mut r, "session", Some("abc")));
        assert_eq!(r, PluginRoute::Session("abc".to_string()));
    }

    #[test]
    fn session_needs_id() {
        let mut r = PluginRoute::Home;
        assert!(!navigate(&mut r, "session", None));
        assert_eq!(r, PluginRoute::Home);
        assert!(!navigate(&mut r, "session", Some("")));
        assert_eq!(r, PluginRoute::Home);
    }

    #[test]
    fn plugin_fallback() {
        let mut r = PluginRoute::Home;
        assert!(navigate(&mut r, "my-plugin", None));
        assert_eq!(r, PluginRoute::Plugin("my-plugin".to_string()));
    }

    #[test]
    fn names() {
        assert_eq!(current_name(&PluginRoute::Home), "home");
        assert_eq!(
            current_name(&PluginRoute::Session("s".to_string())),
            "session"
        );
        assert_eq!(
            current_name(&PluginRoute::Plugin("p".to_string())),
            "plugin"
        );
    }

    #[test]
    fn trunc() {
        let long = "a".repeat(200);
        let mut r = PluginRoute::Home;
        assert!(navigate(&mut r, "session", Some(&long)));
        match &r {
            PluginRoute::Session(id) => assert_eq!(id.chars().count(), MAX_ROUTE_ID),
            other => panic!("expected session, got {other:?}"),
        }
        assert_eq!(map_option_label(&long).chars().count(), MAX_OPTION_LABEL);
    }
}
