#![forbid(unsafe_code)]

//! Phased CLI wiring order for orchestrator (FIX-38).
//!
//! ~530/535 bridge mods dead to tui_entry; wire in phases.
//! ponytail: const table, no dispatch; real wiring in orchestrator.

/// Wiring phases in order: paint, input, live.
pub const PHASES: [&str; 3] = ["paint", "input", "live"];

/// Phase of a known symbol; unknown symbols are "other".
#[must_use]
pub fn phase_of(symbol: &str) -> &str {
    match symbol {
        "split_row" => PHASES[0],
        "EventBus" => PHASES[1],
        "SdkStream" => PHASES[2],
        _ => "other",
    }
}

/// Number of wiring phases.
#[must_use]
pub const fn phase_count() -> usize {
    PHASES.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_phases_in_order() {
        assert_eq!(PHASES, ["paint", "input", "live"]);
        assert_eq!(phase_count(), 3);
    }

    #[test]
    fn paint_symbol() {
        assert_eq!(phase_of("split_row"), "paint");
    }

    #[test]
    fn input_symbol() {
        assert_eq!(phase_of("EventBus"), "input");
    }

    #[test]
    fn live_symbol() {
        assert_eq!(phase_of("SdkStream"), "live");
    }

    #[test]
    fn unknown_is_other() {
        assert_eq!(phase_of("Nope"), "other");
        assert_eq!(phase_of(""), "other");
    }
}
