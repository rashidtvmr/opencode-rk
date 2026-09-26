#![forbid(unsafe_code)]
//! Short model/provider helpers (TS: `packages/tui/src/util/model.ts` `parse`).

/// Cap for [`model_short`] output chars.
pub const MAX_SHORT_LEN: usize = 64;
/// Cap for [`provider_of`] output chars.
pub const MAX_PROVIDER_LEN: usize = 32;
/// Fallback provider when id has no `/` or `:` prefix.
pub const DEFAULT_PROVIDER: &str = "default";

/// Text after last `/` or `:` (whole id when neither), capped at 64 chars.
#[must_use]
pub fn model_short(id: &str) -> String {
    let tail = id.rsplit(['/', ':']).next().unwrap_or("");
    tail.chars().take(MAX_SHORT_LEN).collect()
}

/// Text before first `/` or `:` (`"default"` when absent/empty), capped at 32 chars.
#[must_use]
pub fn provider_of(id: &str) -> String {
    let sep = id.find(['/', ':']);
    let Some(pos) = sep else {
        return DEFAULT_PROVIDER.to_string();
    };
    let head = &id[..pos];
    if head.is_empty() {
        return DEFAULT_PROVIDER.to_string();
    }
    head.chars().take(MAX_PROVIDER_LEN).collect()
}

/// True iff non-empty and contains at least one ASCII alphanumeric.
#[must_use]
pub fn is_valid_model(id: &str) -> bool {
    !id.is_empty() && id.chars().any(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_after_slash() {
        assert_eq!(model_short("anthropic/claude-sonnet"), "claude-sonnet");
        assert_eq!(model_short("a/b/c"), "c");
    }

    #[test]
    fn short_after_colon_and_bare() {
        assert_eq!(model_short("openai:gpt-4"), "gpt-4");
        assert_eq!(model_short("solo"), "solo");
        assert_eq!(model_short(""), "");
    }

    #[test]
    fn short_caps_64() {
        assert_eq!(model_short(&"m".repeat(100)).len(), MAX_SHORT_LEN);
        assert_eq!(
            model_short(&format!("p/{}", "m".repeat(100))).len(),
            MAX_SHORT_LEN
        );
    }

    #[test]
    fn provider_cases() {
        assert_eq!(provider_of("anthropic/claude"), "anthropic");
        assert_eq!(provider_of("solo"), DEFAULT_PROVIDER);
        assert_eq!(provider_of(""), DEFAULT_PROVIDER);
        assert_eq!(provider_of("/m"), DEFAULT_PROVIDER);
    }

    #[test]
    fn valid_needs_alnum() {
        assert!(is_valid_model("p/m"));
        assert!(is_valid_model("solo"));
        assert!(!is_valid_model(""));
        assert!(!is_valid_model(":/"));
    }
}
