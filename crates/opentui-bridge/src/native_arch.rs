#![forbid(unsafe_code)]
//! Native triple matrix for the OpenTUI bridge.
//! Fail-closed link gate lives in build.rs:50-64; this module mirrors its
//! expected artifact names per Rust target triple under native/lib/<triple>.

/// Presence of a vendored native artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactStatus {
    Present,
    Missing,
}

/// One supported target triple and its vendored artifact filenames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeTriple {
    pub triple: &'static str,
    pub static_artifact: &'static str,
    pub shared_artifact: &'static str,
}

/// Supported triples. Unix prefers the static artifact; Windows needs the
/// import library plus the runtime DLL beside the executable.
pub const MATRIX: &[NativeTriple] = &[
    NativeTriple {
        triple: "x86_64-unknown-linux-gnu",
        static_artifact: "libopentui.a",
        shared_artifact: "libopentui.so",
    },
    NativeTriple {
        triple: "aarch64-unknown-linux-gnu",
        static_artifact: "libopentui.a",
        shared_artifact: "libopentui.so",
    },
    NativeTriple {
        triple: "x86_64-apple-darwin",
        static_artifact: "libopentui.a",
        shared_artifact: "libopentui.dylib",
    },
    NativeTriple {
        triple: "aarch64-apple-darwin",
        static_artifact: "libopentui.a",
        shared_artifact: "libopentui.dylib",
    },
    NativeTriple {
        triple: "x86_64-pc-windows-msvc",
        static_artifact: "opentui.lib",
        shared_artifact: "opentui.dll",
    },
    NativeTriple {
        triple: "x86_64-pc-windows-gnu",
        static_artifact: "libopentui.dll.a",
        shared_artifact: "opentui.dll",
    },
];

/// Primary link artifact for a triple (shared name; import lib on Windows).
pub fn artifact_name(triple: &str) -> &'static str {
    MATRIX
        .iter()
        .find(|t| t.triple == triple)
        .map_or("unknown", |t| {
            if t.triple.contains("-pc-windows-") {
                t.static_artifact
            } else {
                t.shared_artifact
            }
        })
}

/// Fail-closed hint mirroring the build.rs:50-64 panic text.
pub fn missing_message(triple: &str) -> String {
    let expected = if triple.ends_with("-apple-darwin") {
        "libopentui.a or libopentui.dylib"
    } else if triple.contains("unknown-linux-gnu") {
        "libopentui.a or libopentui.so"
    } else if triple.ends_with("-pc-windows-msvc") {
        "opentui.lib and opentui.dll"
    } else if triple.contains("-pc-windows-") {
        "libopentui.dll.a and opentui.dll"
    } else {
        "a supported libopentui static/shared artifact"
    };
    format!(
        "native libopentui artifact missing for target '{triple}'.\nexpected {expected} in native/lib/{triple}.\nBuild the pinned OpenTUI native library for this target and vendor it under native/lib/{triple}."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_shared_names() {
        assert_eq!(artifact_name("x86_64-unknown-linux-gnu"), "libopentui.so");
        assert_eq!(artifact_name("aarch64-unknown-linux-gnu"), "libopentui.so");
    }

    #[test]
    fn macos_shared_names() {
        assert_eq!(artifact_name("x86_64-apple-darwin"), "libopentui.dylib");
        assert_eq!(artifact_name("aarch64-apple-darwin"), "libopentui.dylib");
    }

    #[test]
    fn windows_import_names() {
        assert_eq!(artifact_name("x86_64-pc-windows-msvc"), "opentui.lib");
        assert_eq!(artifact_name("x86_64-pc-windows-gnu"), "libopentui.dll.a");
    }

    #[test]
    fn unknown_triple() {
        assert_eq!(artifact_name("riscv64-unknown-linux-gnu"), "unknown");
    }

    #[test]
    fn missing_linux_text() {
        let m = missing_message("x86_64-unknown-linux-gnu");
        assert!(m.contains("libopentui.a or libopentui.so"));
        assert!(m.contains("native/lib/x86_64-unknown-linux-gnu"));
    }

    #[test]
    fn missing_macos_text() {
        let m = missing_message("aarch64-apple-darwin");
        assert!(m.contains("libopentui.a or libopentui.dylib"));
    }

    #[test]
    fn missing_windows_text() {
        let msvc = missing_message("x86_64-pc-windows-msvc");
        assert!(msvc.contains("opentui.lib and opentui.dll"));
        let gnu = missing_message("x86_64-pc-windows-gnu");
        assert!(gnu.contains("libopentui.dll.a and opentui.dll"));
    }

    #[test]
    fn matrix_covers_five_targets() {
        assert!(MATRIX.len() >= 5);
        assert!(MATRIX.iter().any(|t| t.triple == "aarch64-apple-darwin"));
        assert_eq!(ArtifactStatus::Present, ArtifactStatus::Present);
        assert_ne!(ArtifactStatus::Present, ArtifactStatus::Missing);
    }
}
