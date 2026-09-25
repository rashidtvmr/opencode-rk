#![forbid(unsafe_code)]
//! ThemeJson color-spec resolution compatible with the TypeScript theme format.
//!
//! The resolver deliberately returns the resolved source spec rather than RGBA.
//! The caller already owns literal/ANSI conversion in `theme_registry.rs`.

/// Default `thinkingOpacity` when a theme omits it.
pub const DEFAULT_THINKING_OPACITY: f32 = 0.6;

/// Failure resolving a ThemeJson color specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// A reference chain contains the same name twice.
    Circular(String),
    /// A reference is absent from both definitions and theme values.
    Missing(String),
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Circular(chain) => write!(f, "Circular color reference: {chain}"),
            Self::Missing(name) => {
                write!(f, "Color reference \"{name}\" not found in defs or theme")
            }
        }
    }
}

impl std::error::Error for ResolveError {}

fn variant_spec<'a>(spec: &'a str, mode_dark: bool) -> Option<&'a str> {
    let (dark, light) = spec.split_once(",light:")?;
    dark.strip_prefix("dark:")
        .map(|value| if mode_dark { value } else { light })
}

fn resolve_color_inner(
    spec: &str,
    defs: &[(&str, &str)],
    theme: &[(&str, &str)],
    mode_dark: bool,
    chain: &mut Vec<String>,
) -> Result<String, ResolveError> {
    if spec == "transparent"
        || spec == "none"
        || spec.starts_with('#')
        || (!spec.is_empty() && spec.bytes().all(|b| b.is_ascii_digit()))
    {
        return Ok(spec.to_string());
    }

    if let Some(value) = variant_spec(spec, mode_dark) {
        return resolve_color_inner(value, defs, theme, mode_dark, chain);
    }

    if chain.iter().any(|name| name == spec) {
        chain.push(spec.to_string());
        return Err(ResolveError::Circular(chain.join(" -> ")));
    }

    let next = defs
        .iter()
        .find_map(|(name, value)| (*name == spec).then_some(*value))
        .or_else(|| {
            theme
                .iter()
                .find_map(|(name, value)| (*name == spec).then_some(*value))
        })
        .ok_or_else(|| ResolveError::Missing(spec.to_string()))?;

    chain.push(spec.to_string());
    resolve_color_inner(next, defs, theme, mode_dark, chain)
}

/// Resolve a literal, numeric ANSI spec, mode variant, or defs/theme reference.
///
/// References use `defs[name] ?? theme[name]`. Resolution is fail-closed for
/// missing names and circular chains.
#[must_use]
pub fn resolve_color(
    spec: &str,
    defs: &[(&str, &str)],
    theme: &[(&str, &str)],
    mode_dark: bool,
) -> Result<String, ResolveError> {
    resolve_color_inner(spec, defs, theme, mode_dark, &mut Vec::new())
}

/// Resolve `selectedListItemText`, falling back to `background`.
#[must_use]
pub fn resolve_selected_list_item_text(
    value: Option<&str>,
    defs: &[(&str, &str)],
    theme: &[(&str, &str)],
    mode_dark: bool,
) -> Result<String, ResolveError> {
    match value {
        Some(spec) => resolve_color(spec, defs, theme, mode_dark),
        None => resolve_color(
            theme
                .iter()
                .find_map(|(name, value)| (*name == "background").then_some(*value))
                .ok_or_else(|| ResolveError::Missing("background".to_string()))?,
            defs,
            theme,
            mode_dark,
        ),
    }
}

/// Resolve `backgroundMenu`, falling back to `backgroundElement`.
#[must_use]
pub fn resolve_background_menu(
    value: Option<&str>,
    defs: &[(&str, &str)],
    theme: &[(&str, &str)],
    mode_dark: bool,
) -> Result<String, ResolveError> {
    match value {
        Some(spec) => resolve_color(spec, defs, theme, mode_dark),
        None => resolve_color(
            theme
                .iter()
                .find_map(|(name, value)| (*name == "backgroundElement").then_some(*value))
                .ok_or_else(|| ResolveError::Missing("backgroundElement".to_string()))?,
            defs,
            theme,
            mode_dark,
        ),
    }
}

/// Read `thinkingOpacity` from the string-backed theme table, defaulting to `0.6`.
#[must_use]
pub fn thinking_opacity(theme: &[(&str, &str)]) -> f32 {
    theme
        .iter()
        .find_map(|(name, value)| (*name == "thinkingOpacity").then_some(*value))
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_THINKING_OPACITY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_selects_dark_or_light() {
        let spec = "dark:#fff,light:#000";
        assert_eq!(resolve_color(spec, &[], &[], true).unwrap(), "#fff");
        assert_eq!(resolve_color(spec, &[], &[], false).unwrap(), "#000");
    }

    #[test]
    fn resolves_defs_then_theme_chain() {
        let theme = [("surface", "accent"), ("accent", "42")];
        assert_eq!(
            resolve_color("surface", &[("accent", "#123456")], &theme, true).unwrap(),
            "#123456"
        );
        assert_eq!(resolve_color("surface", &[], &theme, true).unwrap(), "42");
        assert_eq!(resolve_color("9", &[], &[], true).unwrap(), "9");
    }

    #[test]
    fn circular_reference_fails_closed() {
        let defs = [("a", "b"), ("b", "a")];
        assert_eq!(
            resolve_color("a", &defs, &[], true),
            Err(ResolveError::Circular("a -> b -> a".to_string()))
        );
    }

    #[test]
    fn missing_reference_fails_closed() {
        assert_eq!(
            resolve_color("missing", &[], &[], true),
            Err(ResolveError::Missing("missing".to_string()))
        );
    }

    #[test]
    fn optional_colors_use_compatibility_fallbacks() {
        let theme = [
            ("background", "dark:bg,light:base"),
            ("backgroundElement", "element"),
        ];
        let defs = [("base", "#eee"), ("bg", "#111"), ("element", "#456")];
        assert_eq!(
            resolve_selected_list_item_text(None, &defs, &theme, true).unwrap(),
            "#111"
        );
        assert_eq!(
            resolve_selected_list_item_text(Some("text"), &[("text", "#123")], &theme, true)
                .unwrap(),
            "#123"
        );
        assert_eq!(
            resolve_background_menu(None, &defs, &theme, true).unwrap(),
            "#456"
        );
    }

    #[test]
    fn thinking_opacity_defaults_to_point_six() {
        assert_eq!(thinking_opacity(&[]), 0.6);
        assert_eq!(thinking_opacity(&[("thinkingOpacity", "0.25")]), 0.25);
    }
}
