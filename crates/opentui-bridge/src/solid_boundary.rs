#![forbid(unsafe_code)]
//! Evidence-backed boundary inventory for the Solid compatibility seam.
//!
//! The TS checkout is unvendored. These names describe the observed boundary,
//! not a native Solid runtime. `solid_host.rs` explicitly keeps reactivity and
//! rendering on the TS side. Its `HostCaps::all_used()` names are the only
//! capability names below treated as Rust-provided; `lib.rs:69` exports the
//! module, not additional Solid APIs. Local helpers such as `TimeToFirstDraw`
//! and `SlotId` are not API coverage evidence.

/// APIs observed in the TS boundary.
pub const USED_BY_TS: &[&str] = &[
    "render",
    "useRenderer",
    "TimeToFirstDraw",
    "extend",
    "createSignal",
    "createMemo",
    "createEffect",
    "createStore",
];

/// Names enabled by `solid_host::HostCaps::all_used()`.
///
/// The `solid_host` module export is intentionally not listed as an API name.
pub const PROVIDED_BY_RUST: &[&str] = &["slots", "portal", "keyboard", "dimensions"];

/// TS-used names absent from the Rust-provided capability inventory.
pub const GAP: &[&str] = &[
    "render",
    "useRenderer",
    "TimeToFirstDraw",
    "extend",
    "createSignal",
    "createMemo",
    "createEffect",
    "createStore",
];

/// Returns the Rust-provided inventory.
#[must_use]
pub const fn all_used() -> &'static [&'static str] {
    PROVIDED_BY_RUST
}

/// Returns the explicit TS/Rust boundary gap.
#[must_use]
pub const fn gap() -> &'static [&'static str] {
    GAP
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn used_by_ts_is_explicit() {
        assert_eq!(
            USED_BY_TS,
            &[
                "render",
                "useRenderer",
                "TimeToFirstDraw",
                "extend",
                "createSignal",
                "createMemo",
                "createEffect",
                "createStore",
            ]
        );
    }

    #[test]
    fn rust_inventory_is_explicit() {
        assert_eq!(all_used(), &["slots", "portal", "keyboard", "dimensions"]);
    }

    #[test]
    fn gap_documents_render_and_use_renderer() {
        assert!(!gap().is_empty());
        assert!(gap().contains(&"render"));
        assert!(gap().contains(&"useRenderer"));
    }

    #[test]
    fn gap_is_the_exact_set_difference() {
        for name in USED_BY_TS {
            assert!(GAP.contains(name), "missing gap entry {name}");
            assert!(
                !PROVIDED_BY_RUST.contains(name),
                "unexpected Rust entry {name}"
            );
        }
        for name in GAP {
            assert!(USED_BY_TS.contains(name), "invented gap entry {name}");
            assert!(
                !PROVIDED_BY_RUST.contains(name),
                "gap overlaps Rust: {name}"
            );
        }
        assert_eq!(GAP.len(), USED_BY_TS.len());
    }

    #[test]
    fn inventory_does_not_invent_slot_ids() {
        for name in USED_BY_TS.iter().chain(PROVIDED_BY_RUST).chain(GAP) {
            assert_ne!(*name, "SlotId");
            assert_ne!(*name, "slotId");
        }
    }
}
