#![forbid(unsafe_code)]
//! Explicit parity gaps. Disclosure only; this module does not alter behavior.

/// Known Rust/TypeScript divergences retained for review and release reporting.
pub const DIVERGENCES: &[(&str, &str)] = &[
    (
        "transcript bodies",
        "Rust flattens bodies into bounded text plus Text/Reasoning/ToolCall; unsupported parts and provider metadata are omitted.",
    ),
    (
        "prompt caps",
        "Rust frecency/history/stash caps are 256/500/16; TypeScript caps are 1000/50/50.",
    ),
    (
        "keymap LIFO",
        "Rust mode changes use a bounded LIFO stack; pop removes the most recently pushed mode and preserves the base mode.",
    ),
    (
        "toast FIFO-vs-single",
        "Rust ToastQueue is bounded FIFO; TypeScript ui/toast keeps one currentToast and show replaces it.",
    ),
    (
        "revert hand parser",
        "Rust hand-parses standard unified diffs; the cited upstream revert-diff utility is absent, and the nearby patch parser is a different format.",
    ),
    (
        "sync stringly",
        "Rust sync accepts stringly event kind and opaque payload, then latches stale; TypeScript receives typed V2Event data and refreshes over the network.",
    ),
    (
        "RenderOptions fps",
        "Rust RenderOptions exposes only optional target_fps with 1..=240 validation; it is a dims-only hint, not full renderer lifecycle options.",
    ),
    (
        "Char validate Ok",
        "Rust accepts WrapMode::Char and text validation returns Ok; the evidenced TypeScript TUI wrap modes are only word and none.",
    ),
];

#[must_use]
pub const fn divergence_count() -> usize {
    DIVERGENCES.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_has_required_minimum() {
        assert!(divergence_count() >= 8);
        assert_eq!(divergence_count(), DIVERGENCES.len());
    }

    #[test]
    fn required_surfaces_are_named() {
        for name in [
            "transcript bodies",
            "prompt caps",
            "keymap LIFO",
            "toast FIFO-vs-single",
            "revert hand parser",
            "sync stringly",
            "RenderOptions fps",
            "Char validate Ok",
        ] {
            assert!(
                DIVERGENCES.iter().any(|(area, _)| *area == name),
                "missing {name}"
            );
        }
    }

    #[test]
    fn disclosures_contain_material_bounds() {
        assert!(DIVERGENCES[1].1.contains("256/500/16"));
        assert!(DIVERGENCES[1].1.contains("1000/50/50"));
        assert!(DIVERGENCES[6].1.contains("1..=240"));
    }
}
