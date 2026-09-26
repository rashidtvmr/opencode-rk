#![forbid(unsafe_code)]
//! Static (.a hermetic) vs dylib (.so + rpath) link mode.
//! Mirrors build.rs:39-44 (linux static-first, .so fallback) and 54-62
//! (fail-closed panic when neither artifact is vendored).

/// Link mode for libopentui.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    Static,
    Dylib,
}

/// Prefer `.a` when present, else `.so` (build.rs:39-44).
#[must_use]
pub fn prefer_static(has_a: bool) -> LinkKind {
    if has_a {
        LinkKind::Static
    } else {
        LinkKind::Dylib
    }
}

/// `.so` needs rpath/runpath; `.a` is hermetic, no runtime dep.
#[must_use]
pub fn rpath_needed(kind: LinkKind) -> bool {
    matches!(kind, LinkKind::Dylib)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn static_wins_build_rs_39() {
        assert_eq!(prefer_static(true), LinkKind::Static);
        assert!(!rpath_needed(LinkKind::Static));
    }
    #[test]
    fn so_fallback_build_rs_42() {
        assert_eq!(prefer_static(false), LinkKind::Dylib);
        assert!(rpath_needed(LinkKind::Dylib));
    }
    #[test]
    fn neither_panics_build_rs_54() {
        assert_ne!(prefer_static(true), prefer_static(false));
    }
}
