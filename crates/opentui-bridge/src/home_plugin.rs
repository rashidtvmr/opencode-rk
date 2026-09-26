#![forbid(unsafe_code)]
//! Home plugin slot passthroughs (TS checkout a0d9b6c).
//! - `packages/tui/src/feature-plugins/home/footer.tsx:88` `home_footer` (order 100)
//! - `packages/tui/src/feature-plugins/home/tips.tsx:39` `home_bottom` (order 100)
//! Reuses `crate::system_plugins::{ID_HOME_FOOTER, ID_HOME_TIPS, HomeFooter}`
//! and `crate::plugin_slots::{SlotName, SlotRegistry}`; nothing redefined here.

use crate::plugin_slots::{SlotName, SlotRegistry};
use crate::system_plugins::{ID_HOME_FOOTER, ID_HOME_TIPS, HomeFooter};

/// Evidenced host slot names.
pub const HOME_FOOTER_SLOT: &str = "home_footer";
pub const HOME_TIPS_SLOT: &str = "home_bottom";

/// Register footer owner id; `true` on success.
pub fn register_home_footer(registry: &mut SlotRegistry) -> bool {
    registry.register(ID_HOME_FOOTER, HomeFooter::slot()).is_ok()
}

/// Register tips owner id; `true` on success.
pub fn register_home_tips(registry: &mut SlotRegistry) -> bool {
    registry.register(ID_HOME_TIPS, SlotName::HomeBottom).is_ok()
}

/// Registration status for both home slots.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HomePlugin {
    pub footer: bool,
    pub tips: bool,
}

impl HomePlugin {
    #[must_use]
    pub const fn new() -> Self {
        Self { footer: false, tips: false }
    }

    pub fn register(registry: &mut SlotRegistry) -> Self {
        Self { footer: register_home_footer(registry), tips: register_home_tips(registry) }
    }

    #[must_use]
    pub const fn ready(self) -> bool {
        self.footer && self.tips
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_consts_match_registry_names() {
        assert_eq!(HOME_FOOTER_SLOT, SlotName::HomeFooter.as_str());
        assert_eq!(HOME_TIPS_SLOT, SlotName::HomeBottom.as_str());
        assert_eq!(HomeFooter::slot().as_str(), HOME_FOOTER_SLOT);
    }

    #[test]
    fn footer_passthrough_registers_owner() {
        let mut r = SlotRegistry::new();
        assert!(register_home_footer(&mut r));
        let got = r.get(SlotName::HomeFooter);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].plugin_id, ID_HOME_FOOTER);
    }

    #[test]
    fn tips_passthrough_registers_owner() {
        let mut r = SlotRegistry::new();
        assert!(register_home_tips(&mut r));
        let got = r.get(SlotName::HomeBottom);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].plugin_id, ID_HOME_TIPS);
    }

    #[test]
    fn plugin_status_ready() {
        let mut r = SlotRegistry::new();
        let p = HomePlugin::register(&mut r);
        assert_eq!(p, HomePlugin { footer: true, tips: true });
        assert!(p.ready());
        assert!(!HomePlugin::new().ready());
    }
}
