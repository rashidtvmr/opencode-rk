#![forbid(unsafe_code)]
//! Full focus-zone state (BRIDGE-PAR-255).

/// Owned focus zone with switch count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusFull {
    zone: String,
    count: u32,
}

impl Default for FocusFull {
    fn default() -> Self {
        Self {
            zone: String::from("chat"),
            count: 0,
        }
    }
}

impl FocusFull {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn valid(zone: &str) -> bool {
        matches!(zone, "chat" | "palette" | "sidebar") && zone.len() <= 32
    }

    pub fn focus(&mut self, zone: &str) -> bool {
        if !Self::valid(zone) || zone.len() > 32 {
            return false;
        }
        if self.zone.as_str() != zone {
            self.zone = zone.to_string();
            self.count = self.count.saturating_add(1);
        }
        true
    }

    #[must_use]
    pub fn zone_of(&self) -> &str {
        &self.zone
    }

    #[must_use]
    pub fn count(&self) -> u32 {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_chat() {
        let f = FocusFull::new();
        assert_eq!(f.zone_of(), "chat");
        assert_eq!(f.count(), 0);
    }

    #[test]
    fn focus_palette_ok() {
        let mut f = FocusFull::new();
        assert!(f.focus("palette"));
        assert_eq!(f.zone_of(), "palette");
        assert_eq!(f.count(), 1);
    }

    #[test]
    fn same_zone_no_bump() {
        let mut f = FocusFull::new();
        assert!(f.focus("chat"));
        assert_eq!(f.count(), 0);
    }

    #[test]
    fn invalid_rejected() {
        let mut f = FocusFull::new();
        assert!(!f.focus("dialog"));
        assert!(!f.focus(""));
        assert_eq!(f.zone_of(), "chat");
        assert_eq!(f.count(), 0);
    }

    #[test]
    fn cap_32_enforced() {
        let mut f = FocusFull::new();
        let long = "a".repeat(33);
        assert!(!f.focus(&long));
        assert_eq!(f.count(), 0);
    }

    #[test]
    fn count_tracks_switches() {
        let mut f = FocusFull::new();
        f.focus("sidebar");
        f.focus("palette");
        f.focus("chat");
        assert_eq!(f.count(), 3);
        assert_eq!(f.zone_of(), "chat");
    }
}
