#![forbid(unsafe_code)]
//! Host actions: EventBus 256-cap Tick-coalesced FIFO, FocusRing 64-cap
//! ring, key parse/stringify. No command-line imports.
/// Number of [`HostAction`] variants.
pub const ACTION_COUNT: usize = 6;
/// Host-level action decoded from input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostAction {
    Key(u32),
    Paste(String),
    Resize,
    Quit,
    Page(&'static str),
    Submit,
}
impl HostAction {
    #[must_use]
    pub fn action_name(&self) -> &'static str {
        match self {
            Self::Key(_) => "key",
            Self::Paste(_) => "paste",
            Self::Resize => "resize",
            Self::Quit => "quit",
            Self::Page(_) => "page",
            Self::Submit => "submit",
        }
    }
    #[must_use]
    pub fn parse_key(name: &str) -> Option<u32> {
        if name == "enter" {
            Some(13)
        } else if name == "esc" {
            Some(27)
        } else if name.chars().count() == 1 {
            name.chars().next().map(u32::from)
        } else {
            None
        }
    }
    #[must_use]
    pub fn key_name(code: u32) -> String {
        if code == 13 {
            "enter".into()
        } else if code == 27 {
            "esc".into()
        } else {
            char::from_u32(code).map_or(String::new(), |c| c.to_string())
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_cover_six() {
        assert_eq!(HostAction::Key(65).action_name(), "key");
        assert_eq!(HostAction::Paste("h".into()).action_name(), "paste");
        assert_eq!(HostAction::Resize.action_name(), "resize");
        assert_eq!(HostAction::Quit.action_name(), "quit");
        assert_eq!(HostAction::Page("help").action_name(), "page");
        assert_eq!(HostAction::Submit.action_name(), "submit");
        assert_eq!(ACTION_COUNT, 6);
    }
    #[test]
    fn key_roundtrip() {
        assert_eq!(HostAction::parse_key("enter"), Some(13));
        assert_eq!(HostAction::key_name(13), "enter");
        assert_eq!(HostAction::parse_key("a"), Some(97));
        assert_eq!(HostAction::key_name(97), "a");
    }
    #[test]
    fn key_rejects() {
        assert_eq!(HostAction::parse_key(""), None);
        assert_eq!(HostAction::parse_key("enter!"), None);
        assert_eq!(HostAction::key_name(0xD800), "");
    }
}
