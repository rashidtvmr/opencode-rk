//! Ops health-check slice (OPS-008).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Health {
    pub ok: bool,
    pub checked: u64,
}
pub fn check_health(failures: u64, checked: u64) -> Health {
    Health { ok: failures == 0, checked }
}
pub fn is_healthy(h: &Health) -> bool {
    h.ok
}
