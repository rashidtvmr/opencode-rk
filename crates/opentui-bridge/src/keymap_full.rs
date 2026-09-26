#![forbid(unsafe_code)]
//! preventDefault/fallthrough stroke wiring.
//!
//! Compact marker form over [`crate::keymap`] str bindings and the object
//! form in [`crate::keybind_tables`]: `"!"` prefix sets `prevent_default`,
//! `"?"` suffix sets `fallthrough`, remainder is the key.

/// Max chars for a [`KeyStroke`] key. Fail-closed: longer rejected.
pub const MAX_STROKE_LEN: usize = 32;

/// Single stroke with dispatch flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyStroke {
    pub key: String,
    pub prevent_default: bool,
    pub fallthrough: bool,
}

/// Parse `"[!]key[?]"`. Empty (or empty remainder) errs.
pub fn parse_stroke(s: &str) -> Result<KeyStroke, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty stroke".to_string());
    }
    let (rest, prevent_default) = match s.strip_prefix('!') {
        Some(r) => (r, true),
        None => (s, false),
    };
    let (rest, fallthrough) = match rest.strip_suffix('?') {
        Some(r) => (r, true),
        None => (rest, false),
    };
    if rest.is_empty() {
        return Err("empty stroke".to_string());
    }
    if rest.len() > MAX_STROKE_LEN {
        return Err("stroke too long".to_string());
    }
    Ok(KeyStroke {
        key: rest.to_string(),
        prevent_default,
        fallthrough,
    })
}

/// Resolve `query` to a stroke index. First key match wins, unless it sets
/// `fallthrough` and a later match exists, then advance (chained).
#[must_use]
pub fn resolve_with_fallthrough(strokes: &[KeyStroke], query: &str) -> Option<usize> {
    let hits: Vec<usize> = strokes
        .iter()
        .enumerate()
        .filter(|(_, s)| s.key == query)
        .map(|(i, _)| i)
        .collect();
    if hits.is_empty() {
        return None;
    }
    let mut cur = 0;
    while strokes[hits[cur]].fallthrough && cur + 1 < hits.len() {
        cur += 1;
    }
    Some(hits[cur])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plain() {
        let s = parse_stroke("ctrl+c").unwrap();
        assert_eq!(s.key, "ctrl+c");
        assert!(!s.prevent_default);
        assert!(!s.fallthrough);
    }

    #[test]
    fn parse_markers() {
        let s = parse_stroke("!ctrl+c?").unwrap();
        assert_eq!(s.key, "ctrl+c");
        assert!(s.prevent_default);
        assert!(s.fallthrough);
        assert!(parse_stroke("!a").unwrap().prevent_default);
        assert!(parse_stroke("b?").unwrap().fallthrough);
    }

    #[test]
    fn parse_empty_errs() {
        assert!(parse_stroke("").is_err());
        assert!(parse_stroke("   ").is_err());
        assert!(parse_stroke("!").is_err());
        assert!(parse_stroke("?").is_err());
        assert!(parse_stroke(&"x".repeat(MAX_STROKE_LEN + 1)).is_err());
    }

    #[test]
    fn resolve_first() {
        let v = vec![parse_stroke("a").unwrap(), parse_stroke("a").unwrap()];
        assert_eq!(resolve_with_fallthrough(&v, "a"), Some(0));
    }

    #[test]
    fn fallthrough_continues() {
        let v = vec![parse_stroke("a?").unwrap(), parse_stroke("a").unwrap()];
        assert_eq!(resolve_with_fallthrough(&v, "a"), Some(1));
        // No later match: stays on first.
        let v2 = vec![parse_stroke("a?").unwrap(), parse_stroke("b").unwrap()];
        assert_eq!(resolve_with_fallthrough(&v2, "a"), Some(0));
    }

    #[test]
    fn no_match_none() {
        let v = vec![parse_stroke("a").unwrap()];
        assert_eq!(resolve_with_fallthrough(&v, "z"), None);
        assert_eq!(resolve_with_fallthrough(&[], "a"), None);
    }
}
