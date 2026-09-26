#![forbid(unsafe_code)]
//! Host routes bypassed by poll_tick % 20: loop drives only DaemonLive/Down +
//! Resize + Key(0x03) + Submit; poll ticks bypass routing; onboarding skipped.

/// Total routes; equals variant count.
pub const ROUTE_COUNT: usize = 10;

/// Host-side route classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostRoute {
    Key,
    Resize,
    Paste,
    Submit,
    TokenDelta,
    TimelineItem,
    DaemonLive,
    Down,
    OnboardingAdvance,
    Error,
}

/// Stable snake_case name.
#[must_use]
pub fn route_name(r: HostRoute) -> &'static str {
    match r {
        HostRoute::Key => "key",
        HostRoute::Resize => "resize",
        HostRoute::Paste => "paste",
        HostRoute::Submit => "submit",
        HostRoute::TokenDelta => "token_delta",
        HostRoute::TimelineItem => "timeline_item",
        HostRoute::DaemonLive => "daemon_live",
        HostRoute::Down => "down",
        HostRoute::OnboardingAdvance => "onboarding_advance",
        HostRoute::Error => "error",
    }
}

/// Poll ticks bypass routing.
#[must_use]
pub fn poll_bypass() -> bool {
    true
}

/// Onboarding skipped; never routed.
#[must_use]
pub fn skip_onboarding() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn count_matches_variants() {
        assert_eq!(ROUTE_COUNT, 10);
    }
    #[test]
    fn names_distinct_nonempty() {
        let all = [
            HostRoute::Key,
            HostRoute::Resize,
            HostRoute::Paste,
            HostRoute::Submit,
            HostRoute::TokenDelta,
            HostRoute::TimelineItem,
            HostRoute::DaemonLive,
            HostRoute::Down,
            HostRoute::OnboardingAdvance,
            HostRoute::Error,
        ];
        assert_eq!(all.len(), ROUTE_COUNT);
        for (i, a) in all.iter().enumerate() {
            assert!(!route_name(*a).is_empty());
            for b in &all[..i] {
                assert_ne!(route_name(*a), route_name(*b));
            }
        }
        assert_eq!(route_name(HostRoute::DaemonLive), "daemon_live");
    }
    #[test]
    fn bypass_and_skip() {
        assert!(poll_bypass());
        assert!(skip_onboarding());
    }
    #[test]
    fn spot_names() {
        assert_eq!(route_name(HostRoute::Key), "key");
        assert_eq!(route_name(HostRoute::Error), "error");
    }
}
