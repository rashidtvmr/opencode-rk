#![forbid(unsafe_code)]
//! Connected predicate kept beside `Connected` bool flag.
//! Port of `use-connected.tsx:4-12`: TS memo is true when a provider id
//! is usable, i.e. non-empty and neither `"opencode"` nor `"cost"`.
//! `use_connected_full::Connected` stores only the resulting bool, so it
//! drops this predicate; call `is_connected` before `Connected::set`.

/// True when `id` marks a connected provider.
#[must_use]
pub fn is_connected(id: &str) -> bool {
    !id.is_empty() && id != "opencode" && id != "cost"
}

/// Note documenting the predicate dropped by the bool flag.
#[allow(non_snake_case)]
#[must_use]
pub fn PRED_NOTE() -> &'static str {
    "bool drops id != opencode/cost; see use-connected.tsx:4-12"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_offline() {
        assert!(!is_connected(""));
    }

    #[test]
    fn opencode_is_offline() {
        assert!(!is_connected("opencode"));
    }

    #[test]
    fn cost_is_offline() {
        assert!(!is_connected("cost"));
    }

    #[test]
    fn other_is_online() {
        assert!(is_connected("anthropic"));
    }

    #[test]
    fn note_names_source() {
        assert!(PRED_NOTE().contains("use-connected.tsx:4-12"));
    }
}
