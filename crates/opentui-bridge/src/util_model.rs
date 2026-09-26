#![forbid(unsafe_code)]
//! Model id helpers (TS: `packages/tui/src/util/model.ts`).
//!
//! TS `parse` splits on first `/`: provider = first segment, model = rest
//! joined with `/`. Bare ids (no `/`) yield empty provider here so label and
//! validity stay fail-closed. See `model_ref.rs` for the struct/index form.

/// Max accepted id length (fail-closed bound).
pub const MAX_MODEL_ID_LEN: usize = 256;

/// Split `provider/model` on first `/`; missing `/` -> `("", id)`.
#[must_use]
pub fn split_model(id: &str) -> (String, String) {
    match id.find('/') {
        Some(i) => (id[..i].to_string(), id[i + 1..].to_string()),
        None => (String::new(), id.to_string()),
    }
}

/// Display label: `"model (provider)"`, or bare id when no provider.
#[must_use]
pub fn model_label(id: &str) -> String {
    let (provider, model) = split_model(id);
    if provider.is_empty() {
        model
    } else {
        format!("{model} ({provider})")
    }
}

/// Valid iff both parts non-empty and total len within bound.
#[must_use]
pub fn is_valid_model(id: &str) -> bool {
    if id.is_empty() || id.len() > MAX_MODEL_ID_LEN {
        return false;
    }
    let (provider, model) = split_model(id);
    !provider.is_empty() && !model.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_ok() {
        assert_eq!(
            split_model("anthropic/claude"),
            ("anthropic".into(), "claude".into())
        );
    }

    #[test]
    fn split_keeps_extra_slashes() {
        assert_eq!(split_model("a/b/c"), ("a".into(), "b/c".into()));
    }

    #[test]
    fn split_no_slash_empty_provider() {
        assert_eq!(split_model("solo"), ("".into(), "solo".into()));
    }

    #[test]
    fn label_format() {
        assert_eq!(model_label("anthropic/claude"), "claude (anthropic)");
        assert_eq!(model_label("solo"), "solo");
    }

    #[test]
    fn invalid_empty_and_bare() {
        assert!(!is_valid_model(""));
        assert!(!is_valid_model("solo"));
        assert!(!is_valid_model("p/"));
        assert!(!is_valid_model("/m"));
    }

    #[test]
    fn invalid_overlong() {
        let long = format!("p/{}", "m".repeat(MAX_MODEL_ID_LEN));
        assert!(!is_valid_model(&long));
        assert!(is_valid_model("p/m"));
    }
}
