#![forbid(unsafe_code)]
//! Full triple matrix: names only, no link logic.
//! Mirrors build.rs:19,32-84 gate and native_arch.rs:20-54 MATRIX.

/// (triple, static preferred, shared fallback).
pub const NATIVE_TRIPLES: [(&str, &str, &str); 6] = [
    ("x86_64-unknown-linux-gnu", "libopentui.a", "libopentui.so"),
    ("aarch64-unknown-linux-gnu", "libopentui.a", "libopentui.so"),
    ("x86_64-apple-darwin", "libopentui.a", "libopentui.dylib"),
    ("aarch64-apple-darwin", "libopentui.a", "libopentui.dylib"),
    ("x86_64-pc-windows-msvc", "opentui.lib", "opentui.dll"),
    ("x86_64-pc-windows-gnu", "libopentui.dll.a", "opentui.dll"),
];

pub fn is_triple_supported(triple: &str) -> bool {
    NATIVE_TRIPLES.iter().any(|t| t.0 == triple)
}

pub fn artifact_name(triple: &str, kind: &str) -> &'static str {
    NATIVE_TRIPLES
        .iter()
        .find(|t| t.0 == triple)
        .map_or("unknown", |t| if kind == "static" { t.1 } else { t.2 })
}

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
        "native libopentui artifact missing for target '{triple}'.\nexpected {expected} in native/lib/{triple}."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    // build.rs:19 native/lib/<triple>; native_arch.rs:20-54 MATRIX shape.
    #[test]
    fn matrix_has_six_triples() {
        assert_eq!(NATIVE_TRIPLES.len(), 6);
        assert!(is_triple_supported("x86_64-unknown-linux-gnu"));
        assert!(!is_triple_supported("riscv64-unknown-linux-gnu"));
    }
    // build.rs:32-44 .a preferred, shared fallback; native_arch.rs:20-54 names.
    #[test]
    fn static_preferred_shared_fallback() {
        assert_eq!(
            artifact_name("x86_64-unknown-linux-gnu", "static"),
            "libopentui.a"
        );
        assert_eq!(
            artifact_name("x86_64-unknown-linux-gnu", "shared"),
            "libopentui.so"
        );
        assert_eq!(
            artifact_name("aarch64-apple-darwin", "shared"),
            "libopentui.dylib"
        );
        assert_eq!(
            artifact_name("x86_64-pc-windows-msvc", "static"),
            "opentui.lib"
        );
        assert_eq!(
            artifact_name("x86_64-pc-windows-gnu", "shared"),
            "opentui.dll"
        );
        assert_eq!(artifact_name("nope", "shared"), "unknown");
    }
    // build.rs:50-84 fail-closed text per OS; native_arch.rs:20-54 MATRIX.
    #[test]
    fn missing_message_per_os() {
        let l = missing_message("x86_64-unknown-linux-gnu");
        assert!(l.contains("libopentui.a or libopentui.so"));
        let m = missing_message("x86_64-apple-darwin");
        assert!(m.contains("libopentui.a or libopentui.dylib"));
        let w = missing_message("x86_64-pc-windows-gnu");
        assert!(w.contains("libopentui.dll.a and opentui.dll"));
    }
}
