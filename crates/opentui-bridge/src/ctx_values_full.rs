#![forbid(unsafe_code)]

//! Bounded scalar ctx value (22 engines scalar-only, no values upstream).
//! Gap: keys exist without values; no hydrate/refresh wired here.
//! ponytail: fixed caps, no store/eviction; add when hydration needed.

/// Max chars for value.
pub const V_MAX: usize = 512;
/// Max chars for key.
pub const K_MAX: usize = 128;

/// Single bounded key/value pair.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CtxVal {
    pub key: String,
    pub val: String,
}

impl CtxVal {
    /// Some when [`put_ok`] holds, else None.
    #[must_use]
    pub fn new(k: &str, v: &str) -> Option<Self> {
        if !put_ok(k, v) {
            return None;
        }
        Some(Self {
            key: k.to_string(),
            val: val_trunc(v),
        })
    }
}

/// True when both non-empty and within char caps.
#[must_use]
pub fn put_ok(k: &str, v: &str) -> bool {
    !k.is_empty() && !v.is_empty() && k.chars().count() <= K_MAX && v.chars().count() <= V_MAX
}

/// Truncate to [`V_MAX`] chars on char boundaries.
#[must_use]
pub fn val_trunc(s: &str) -> String {
    if s.chars().count() <= V_MAX {
        return s.to_string();
    }
    s.chars().take(V_MAX).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok() {
        assert!(put_ok("k", "v"));
        assert!(CtxVal::new("k", "v").is_some());
    }

    #[test]
    fn empty_reject() {
        assert!(!put_ok("", "v"));
        assert!(!put_ok("k", ""));
        assert!(CtxVal::new("", "v").is_none());
    }

    #[test]
    fn caps() {
        assert!(!put_ok(&"k".repeat(129), "v"));
        assert!(!put_ok("k", &"v".repeat(513)));
        assert!(put_ok(&"k".repeat(128), &"v".repeat(512)));
    }

    #[test]
    fn trunc_ascii() {
        assert_eq!(val_trunc(&"v".repeat(600)).len(), 512);
        assert_eq!(val_trunc("abc"), "abc");
    }

    #[test]
    fn trunc_unicode_safe() {
        let s = "e".repeat(500) + &"🦀".repeat(20);
        let t = val_trunc(&s);
        assert_eq!(t.chars().count(), V_MAX);
        assert!(t.is_char_boundary(t.len()));
    }
}
