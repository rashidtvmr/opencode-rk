#![forbid(unsafe_code)]
//! OS info string (mirrors `provider/provider.ts:612` @ a0d9b6c).
//!
//! Spec cited `util/system.ts` (20 lines); absent in checkout. Actual
//! source: User-Agent `` `(${os.platform()} ${os.release()}; ${os.arch()})` ``.
//! Fail-closed: release/arch truncated to [`MAX_FIELD_LEN`] chars.

/// Cap per field.
pub const MAX_FIELD_LEN: usize = 128;

/// OS kinds (`os.platform()` values).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsKind {
    MacOs,
    Windows,
    Linux,
    Other,
}

impl OsKind {
    /// Compile-time detection.
    #[must_use]
    pub fn detect() -> Self {
        #[cfg(target_os = "macos")]
        return Self::MacOs;
        #[cfg(target_os = "windows")]
        return Self::Windows;
        #[cfg(target_os = "linux")]
        return Self::Linux;
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        return Self::Other;
    }

    #[must_use]
    pub fn platform(&self) -> &'static str {
        match self {
            Self::MacOs => "darwin",
            Self::Windows => "win32",
            Self::Linux => "linux",
            Self::Other => "other",
        }
    }
}

fn trunc(s: &str) -> String {
    s.chars().take(MAX_FIELD_LEN).collect()
}

/// TS `` `(${platform} ${release}; ${arch})` `` shape.
#[must_use]
pub fn describe(kind: OsKind, release: &str, arch: &str) -> String {
    format!("({} {}; {})", kind.platform(), trunc(release), trunc(arch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_matches_ts() {
        assert_eq!(describe(OsKind::Linux, "6.1", "x64"), "(linux 6.1; x64)");
    }

    #[test]
    fn platforms() {
        assert_eq!(OsKind::MacOs.platform(), "darwin");
        assert_eq!(OsKind::Windows.platform(), "win32");
        assert_eq!(OsKind::Other.platform(), "other");
    }

    #[test]
    fn fields_truncated() {
        let d = describe(OsKind::Linux, &"r".repeat(300), "x64");
        assert!(d.len() < 300);
    }
}
