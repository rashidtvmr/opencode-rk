#![forbid(unsafe_code)]
//! Platform artifact manifest: triples -> native lib filenames.
//! Mirrors `build.rs` fail-closed gating (panics on missing native artifact,
//! see build.rs:19,26-64). `--features native` only supports linux-x86_64
//! today; other triples return `ManifestError::Unsupported`.

use std::fmt;

const TRIPLES: [(&str, &str); 5] = [
    ("linux-x86_64", "libopentui.so"),
    ("linux-aarch64", "libopentui.so"),
    ("darwin-aarch64", "libopentui.dylib"),
    ("darwin-x86_64", "libopentui.dylib"),
    ("windows-x86_64", "opentui.dll"),
];

const WIN_LIB: &str = "opentui.lib";

/// Fail-closed error for unknown/unsupported triples. Mirrors build.rs panic
/// "native libopentui artifact missing for target" (build.rs:58-64).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    UnsupportedTriple(String),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::UnsupportedTriple(t) => write!(
                f,
                "native libopentui artifact missing for target '{t}'. vendor under native/lib/{t}."
            ),
        }
    }
}

impl std::error::Error for ManifestError {}

#[must_use]
pub fn artifact_filename(t: &str) -> Result<&'static str, ManifestError> {
    TRIPLES
        .iter()
        .find(|(k, _)| *k == t)
        .map(|(_, f)| *f)
        .ok_or_else(|| ManifestError::UnsupportedTriple(t.to_string()))
}

#[must_use]
pub fn artifact_files(t: &str) -> Result<Vec<&'static str>, ManifestError> {
    match t {
        "windows-x86_64" => Ok(vec!["opentui.dll", WIN_LIB]),
        _ if is_supported(t) => Ok(vec![artifact_filename(t)?]),
        other => Err(ManifestError::UnsupportedTriple(other.to_string())),
    }
}

#[must_use]
pub fn is_supported(t: &str) -> bool {
    TRIPLES.iter().any(|(k, _)| *k == t)
}

#[cfg(test)]
mod tests {
    use super::*;

    // build.rs:19 joins native/lib/<triple>; build.rs:26-49 gates .so/.dylib/.lib/.dll.
    #[test]
    fn linux_so_and_x86_64_matches_build_rs_gate() {
        for t in ["linux-x86_64", "linux-aarch64"] {
            assert_eq!(artifact_filename(t).unwrap(), "libopentui.so");
            assert!(is_supported(t));
        }
    }
    #[test]
    fn darwin_dylib_supported() {
        for t in ["darwin-aarch64", "darwin-x86_64"] {
            assert_eq!(artifact_filename(t).unwrap(), "libopentui.dylib");
            assert!(is_supported(t));
        }
    }
    #[test]
    fn windows_has_dll_and_import_lib() {
        assert_eq!(
            artifact_files("windows-x86_64").unwrap(),
            vec!["opentui.dll", WIN_LIB]
        );
        assert!(is_supported("windows-x86_64"));
    }
    // build.rs:50-64 fail-closed _ arm: panic on unsupported triple.
    #[test]
    fn unsupported_triple_fail_closed_error() {
        let err = artifact_filename("freebsd-riscv64").unwrap_err();
        assert_eq!(
            err,
            ManifestError::UnsupportedTriple("freebsd-riscv64".to_string())
        );
        assert!(!is_supported("freebsd-riscv64"));
    }
}
