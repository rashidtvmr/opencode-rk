#![forbid(unsafe_code)]
//! Full Solid capability cut: no-runtime-by-design.
//!
//! | ref | evidence |
//! | app.tsx | render, useRenderer, TTFD |
//! | dialog.tsx | createSignal, createStore |
//! | bg-pulse | extend |
//! | reactivity | createEffect stays TS |

/// TS-owned names cut from native scope by design.
pub const GAP: [&str; 7] = [
    "render",
    "useRenderer",
    "TTFD",
    "extend",
    "createSignal",
    "createEffect",
    "createStore",
];

/// Rust-provided host names.
pub const PROVIDED: [&str; 4] = ["slots", "portal", "keyboard", "dims"];

/// Number of documented gap entries.
#[must_use]
pub const fn gap_count() -> usize {
    GAP.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gap_is_seven() {
        assert_eq!(GAP.len(), 7);
        assert_eq!(gap_count(), 7);
    }

    #[test]
    fn provided_is_four() {
        assert_eq!(PROVIDED, ["slots", "portal", "keyboard", "dims"]);
    }

    #[test]
    fn gap_names_are_exact() {
        assert_eq!(
            GAP,
            [
                "render",
                "useRenderer",
                "TTFD",
                "extend",
                "createSignal",
                "createEffect",
                "createStore",
            ]
        );
    }

    #[test]
    fn gap_disjoint_from_provided() {
        for g in GAP {
            assert!(!PROVIDED.contains(&g));
        }
    }
}
