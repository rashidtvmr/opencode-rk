#![forbid(unsafe_code)]

/// Canonical implementation owner for a duplicated utility surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UtilOwner {
    JsonPersist,
    ToolMeta,
    ErrorFormat,
    Debounce,
}

/// Return the canonical utility name for a legacy symbol or duplicate.
#[must_use]
pub fn canonical_of(name: &str) -> &str {
    match name {
        "json_persist"
        | "json_persist.rs"
        | "persist"
        | "persistence"
        | "brace_scan"
        | "brace-scan"
        | "brace scan"
        | "write_json_atomic"
        | "read_json_atomic"
        | "validate_json_shape" => "json_persist",
        "tool_meta"
        | "tool_meta.rs"
        | "prefix_hack"
        | "prefix-hack"
        | "prefix hack"
        | "structured_map"
        | "structured-map"
        | "structured map"
        | "ToolMeta"
        | "tool_display_metadata" => "tool_meta",
        "error_format" | "error_format.rs" | "TaggedError" | "NativeError" => "error_format",
        "debounce" | "debounce.rs" | "debounce_signal" | "debounce_signal.rs" | "Debounced" => {
            "debounce_signal"
        }
        other => other,
    }
}

/// Resolve a utility name to its one canonical owner.
#[must_use]
pub fn owner_of(name: &str) -> UtilOwner {
    match canonical_of(name) {
        "json_persist" => UtilOwner::JsonPersist,
        "tool_meta" => UtilOwner::ToolMeta,
        "error_format" => UtilOwner::ErrorFormat,
        "debounce_signal" => UtilOwner::Debounce,
        other => panic!("unknown utility owner: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_duplicate_implementations() {
        assert_eq!(canonical_of("persist"), "json_persist");
        assert_eq!(canonical_of("Debounced"), "debounce_signal");
    }

    #[test]
    fn maps_duplicate_symbols_to_their_owner() {
        assert_eq!(owner_of("brace_scan"), UtilOwner::JsonPersist);
        assert_eq!(owner_of("structured_map"), UtilOwner::ToolMeta);
        assert_eq!(owner_of("NativeError"), UtilOwner::ErrorFormat);
        assert_eq!(owner_of("Debounced"), UtilOwner::Debounce);
    }

    #[test]
    fn canonical_names_are_idempotent() {
        for name in [
            "json_persist",
            "tool_meta",
            "error_format",
            "debounce_signal",
        ] {
            assert_eq!(canonical_of(canonical_of(name)), name);
            let _ = owner_of(name);
        }
    }
}
