// Native artifact naming for OpenTUI bridge
// Based on build.rs:50-64 fail-closed gate logic
// Artifact lookup in native/lib/<triple>/

//! Native artifact naming for OpenTUI bridge.
//! Provides artifact names and missing message helpers per Rust target triple.
//! Based on build.rs:50-64 fail-closed gate logic.

/// Status of native artifact presence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactStatus {
    Present,
    Missing,
}

/// Map a Rust target triple to its OpenTUI artifact name.
/// Returns the expected artifact filename (without extension prefix for static libs).
pub fn artifact_name(triple: &str) -> &'static str {
    match triple {
        "x86_64-unknown-linux-gnu" => "libopentui.so",
        "aarch64-unknown-linux-gnu" => "libopentui.so",
        "x86_64-apple-darwin" => "libopentui.dylib",
        "aarch64-apple-darwin" => "libopentui.dylib",
        "x86_64-pc-windows-msvc" => "opentui.lib",
        "i686-pc-windows-msvc" => "opentui.lib",
        _ => "unknown",
    }
}

/// Generate a user-friendly missing message for a target triple.
/// Matches the panic message format from build.rs:50-64.
pub fn missing_message(triple: &str) -> String {
    let expected = match triple {
        "x86_64-apple-darwin" | "aarch64-apple-darwin" => "libopentui.a or libopentui.dylib",
        "x86_64-unknown-linux-gnu" | "aarch64-unknown-linux-gnu" => "libopentui.a or libopentui.so",
        "x86_64-pc-windows-msvc" | "i686-pc-windows-msvc" => "opentui.lib and opentui.dll",
        "i586-unknown-linux-gnu" => "libopentui.a or libopentui.so",
        _ => "a supported libopentui static/shared artifact",
    };
    format!(
        "native libopentui artifact missing for target '{}'.\nexpected {}.",
        triple, expected
    )
}

/// Parse a triple string and return its normalized form.
pub fn parse_triple(triple: &str) -> Option<&'static str> {
    const TRIPLES: &[&str] = &[
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
        "i686-pc-windows-msvc",
    ];
    TRIPLES
        .iter()
        .copied()
        .find(|&t| triple == t || triple.starts_with(t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artifact_name_linux_x86_64() {
        assert_eq!(artifact_name("x86_64-unknown-linux-gnu"), "libopentui.so");
    }

    #[test]
    fn test_artifact_name_linux_aarch64() {
        assert_eq!(artifact_name("aarch64-unknown-linux-gnu"), "libopentui.so");
    }

    #[test]
    fn test_artifact_name_macos_x86_64() {
        assert_eq!(artifact_name("x86_64-apple-darwin"), "libopentui.dylib");
    }

    #[test]
    fn test_artifact_name_macos_aarch64() {
        assert_eq!(artifact_name("aarch64-apple-darwin"), "libopentui.dylib");
    }

    #[test]
    fn test_artifact_name_windows_msvc() {
        assert_eq!(artifact_name("x86_64-pc-windows-msvc"), "opentui.lib");
    }

    #[test]
    fn test_missing_message_linux() {
        let msg = missing_message("x86_64-unknown-linux-gnu");
        assert!(msg.contains("libopentui.so"));
        assert!(msg.contains("x86_64-unknown-linux-gnu"));
    }

    #[test]
    fn test_missing_message_macos() {
        let msg = missing_message("x86_64-apple-darwin");
        assert!(msg.contains("libopentui.dylib"));
        assert!(msg.contains("x86_64-apple-darwin"));
    }

    #[test]
    fn test_missing_message_windows() {
        let msg = missing_message("x86_64-pc-windows-msvc");
        assert!(msg.contains("opentui.lib"));
        assert!(msg.contains("opentui.dll"));
    }

    #[test]
    fn test_parse_triple_valid() {
        assert!(parse_triple("x86_64-unknown-linux-gnu").is_some());
        assert!(parse_triple("aarch64-apple-darwin").is_some());
    }

    #[test]
    fn test_parse_triple_invalid() {
        assert!(parse_triple("invalid-triple").is_none());
    }

    #[test]
    fn test_unknown_triple_artifact() {
        assert_eq!(artifact_name("abcdef"), "unknown");
    }
}
