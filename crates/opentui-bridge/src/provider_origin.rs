#![forbid(unsafe_code)]
//! Console-managed provider check (mirrors `config/config.ts:320,502,591`).
//!
//! Spec cited `util/provider-origin.ts` (7 lines); absent in checkout.
//! Actual source: `consoleManagedProviders: Set<string>` populated from
//! org config keys, exposed via `Array.from`. Fail-closed: empty list or
//! empty id -> `false`.

/// TS `consoleManagedProviders.has(providerID)` equivalent.
#[must_use]
pub fn is_console_managed(managed: &[&str], provider_id: &str) -> bool {
    if provider_id.is_empty() {
        return false;
    }
    managed.iter().any(|m| *m == provider_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_returns_true() {
        assert!(is_console_managed(&["openai", "anthropic"], "openai"));
    }

    #[test]
    fn nonmember_returns_false() {
        assert!(!is_console_managed(&["openai"], "other"));
    }

    #[test]
    fn empty_inputs_false() {
        assert!(!is_console_managed(&[], "openai"));
        assert!(!is_console_managed(&["openai"], ""));
    }
}
