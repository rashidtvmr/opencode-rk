#![forbid(unsafe_code)]
//! OS/platform + shell helpers.
//!
//! TS truth: `packages/tui/src/util/system.ts` (`describeOS` maps
//! darwin/win32/linux + arch; `describeTerminal` reads `TERM_*` env).
//! ponytail: no release-version helper; add `os_release()` when needed.

/// Platform: `macos` | `windows` | `linux` (non-mac/win falls back to `linux`).
#[must_use]
pub fn plat() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/// CPU arch: `x64` (x86_64) | `arm64` (aarch64) | `other`.
#[must_use]
pub fn arch_label() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "other"
    }
}

/// Compile-time fallback shell: `cmd` on Windows, `sh` elsewhere.
fn fallback_shell() -> &'static str {
    if cfg!(windows) {
        "cmd"
    } else {
        "sh"
    }
}

/// Login shell from `$SHELL` when set/non-empty, else the cfg fallback.
// ponytail: leaks one `Box<str>` per distinct `$SHELL`; cache when hot.
#[must_use]
pub fn shell_of() -> &'static str {
    match std::env::var("SHELL") {
        Ok(s) if !s.is_empty() => Box::leak(s.into_boxed_str()),
        _ => fallback_shell(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plat_is_known() {
        assert!(matches!(plat(), "linux" | "macos" | "windows"));
    }

    #[test]
    fn arch_is_known() {
        assert!(matches!(arch_label(), "x64" | "arm64" | "other"));
    }

    #[test]
    fn fallback_is_known() {
        assert!(matches!(fallback_shell(), "sh" | "cmd"));
    }

    #[test]
    fn shell_of_nonempty() {
        assert!(!shell_of().is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn plat_linux() {
        assert_eq!(plat(), "linux");
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn arch_x64() {
        assert_eq!(arch_label(), "x64");
    }
}
