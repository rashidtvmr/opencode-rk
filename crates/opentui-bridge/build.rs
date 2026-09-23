//! Build script: wire the platform-native libopentui when the `native`
//! feature is enabled. Artifacts are vendored per Rust target triple under
//! `native/lib/<triple>`; development and release builds use the same gate.
//!
//! Provenance contract: each `native/lib/<triple>/` directory MUST contain a
//! `PROVENANCE` file with `key=value` lines:
//! `zig_commit`, `build_flags`, `source_pin`, `sha256` (64 lowercase hex of
//! the linked artifact). build.rs validates it and fails closed, listing the
//! expected_artifact for the target.

use std::path::{Path, PathBuf};

/// How the vendored artifact links into the Rust crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkKind {
    Static,
    Dylib,
}

/// Pure artifact-name selection: probe results -> link decision.
/// `windows_import` covers both MSVC (`opentui.lib`) and GNU
/// (`libopentui.dll.a`); the runtime DLL must still be present.
fn select_link_kind(
    target_os: &str,
    static_unix: bool,
    linux_shared: bool,
    mac_shared: bool,
    windows_import: bool,
    windows_runtime: bool,
) -> Option<LinkKind> {
    match target_os {
        "macos" if static_unix => Some(LinkKind::Static),
        "macos" if mac_shared => Some(LinkKind::Dylib),
        "linux" if static_unix => Some(LinkKind::Static),
        "linux" if linux_shared => Some(LinkKind::Dylib),
        "windows" if windows_import && windows_runtime => Some(LinkKind::Dylib),
        _ => None,
    }
}

/// Pure expected-artifact description for the link/gate error paths.
fn expected_artifact(target_os: &str, target_env: &str) -> &'static str {
    match target_os {
        "macos" => "libopentui.a or libopentui.dylib",
        "linux" => "libopentui.a or libopentui.so",
        "windows" if target_env == "msvc" => "opentui.lib and opentui.dll",
        "windows" => "libopentui.dll.a and opentui.dll",
        _ => "a supported libopentui static/shared artifact",
    }
}

/// Parsed provenance record from `native/lib/<triple>/PROVENANCE`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Provenance {
    zig_commit: String,
    build_flags: String,
    source_pin: String,
    sha256: String,
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Pure provenance-file parse/validate: every field required, non-empty,
/// sha256 must be 64 lowercase hex chars. Returns a message on failure.
fn parse_provenance(text: &str) -> Result<Provenance, String> {
    let mut zig_commit: Option<String> = None;
    let mut build_flags: Option<String> = None;
    let mut source_pin: Option<String> = None;
    let mut sha256: Option<String> = None;
    for (idx, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line.split_once('=').ok_or_else(|| {
            format!("provenance: line {} is not key=value: {raw:?}", idx + 1)
        })?;
        let value = value.trim().to_string();
        match key.trim() {
            "zig_commit" => zig_commit = Some(value),
            "build_flags" => build_flags = Some(value),
            "source_pin" => source_pin = Some(value),
            "sha256" => sha256 = Some(value),
            other => {
                return Err(format!("provenance: unknown key {other:?} on line {}", idx + 1));
            }
        }
    }
    let zig_commit = zig_commit.filter(|s| !s.is_empty())
        .ok_or_else(|| "provenance: missing required field zig_commit".to_string())?;
    let build_flags = build_flags.filter(|s| !s.is_empty())
        .ok_or_else(|| "provenance: missing required field build_flags".to_string())?;
    let source_pin = source_pin.filter(|s| !s.is_empty())
        .ok_or_else(|| "provenance: missing required field source_pin".to_string())?;
    let sha256 = sha256.filter(|s| !s.is_empty())
        .ok_or_else(|| "provenance: missing required field sha256".to_string())?;
    if !is_hex64(&sha256) {
        return Err("provenance: sha256 must be 64 lowercase hex chars".to_string());
    }
    Ok(Provenance { zig_commit, build_flags, source_pin, sha256 })
}

/// Pure drift-message formatter: names the triple, the expected_artifact,
/// and the reason so failures are actionable.
fn drift_message(triple: &str, libdir: &Path, expected: &str, reason: &str) -> String {
    format!(
        "native libopentui artifact drift for target '{triple}'.\n\
         expected_artifact: {expected} in {}.\n\
         reason: {reason}\n\
         Rebuild the pinned OpenTUI native library for this target, refresh \
         native/lib/{triple}/PROVENANCE, and vendor it under native/lib/{triple}.",
        libdir.display()
    )
}

fn exists(dir: &Path, name: &str) -> bool {
    dir.join(name).is_file()
}

/// Fail-closed provenance validation for one triple directory: missing file,
/// unreadable file, or invalid content all surface the expected_artifact.
fn validate_provenance_file(libdir: &Path, expected: &str) -> Result<Provenance, String> {
    let path = libdir.join("PROVENANCE");
    let text = std::fs::read_to_string(&path).map_err(|e| {
        format!(
            "provenance file unreadable at {} (expected_artifact: {expected}): {e}",
            path.display()
        )
    })?;
    parse_provenance(&text).map_err(|e| {
        format!("{e} (expected_artifact: {expected} in {})", libdir.display())
    })
}

#[cfg(not(test))]
fn main() {
    if std::env::var("CARGO_FEATURE_NATIVE").is_err() {
        println!("cargo:warning=opentui-bridge: native feature off; skipping link gate");
        return;
    }

    let triple = std::env::var("TARGET").unwrap_or_default();
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into()));
    let libdir = manifest.join("native").join("lib").join(&triple);
    println!("cargo:rustc-link-search=native={}", libdir.display());
    println!("cargo:rerun-if-changed={}", libdir.display());
    println!("cargo:rerun-if-changed={}", libdir.join("PROVENANCE").display());

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    let static_unix = exists(&libdir, "libopentui.a");
    let linux_shared = exists(&libdir, "libopentui.so");
    let mac_shared = exists(&libdir, "libopentui.dylib");
    let windows_import = exists(&libdir, "opentui.lib") || exists(&libdir, "libopentui.dll.a");
    let windows_runtime = exists(&libdir, "opentui.dll");
    let expected = expected_artifact(&target_os, &target_env);

    match select_link_kind(&target_os, static_unix, linux_shared, mac_shared, windows_import, windows_runtime) {
        Some(LinkKind::Static) => {
            println!("cargo:rustc-link-lib=static=opentui");
        }
        Some(LinkKind::Dylib) => {
            println!("cargo:rustc-link-lib=dylib=opentui");
        }
        None => {
            panic!(
                "native libopentui artifact missing for target '{triple}'.\n\
                 expected {expected} in {}.\n\
                 Build the pinned OpenTUI native library for this target and vendor it under native/lib/{triple}.",
                libdir.display()
            );
        }
    }

    // Provenance gate: fail closed on drift or missing record.
    if let Err(reason) = validate_provenance_file(&libdir, expected) {
        panic!("{}", drift_message(&triple, &libdir, expected, &reason));
    }
    println!("cargo:warning=opentui-bridge: provenance validated for {triple}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const GOOD: &str = "zig_commit=0.13.0+abc123\nbuild_flags=-O3 -fPIC\nsource_pin=opentui-v1.2.3\nsha256=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n";

    #[test]
    fn static_preferred_on_macos() {
        assert_eq!(
            select_link_kind("macos", true, false, true, false, false),
            Some(LinkKind::Static)
        );
    }

    #[test]
    fn windows_requires_runtime_dll() {
        assert_eq!(select_link_kind("windows", false, false, false, true, false), None);
        assert_eq!(
            select_link_kind("windows", false, false, false, true, true),
            Some(LinkKind::Dylib)
        );
    }

    #[test]
    fn expected_artifact_names_per_platform() {
        assert_eq!(expected_artifact("macos", ""), "libopentui.a or libopentui.dylib");
        assert_eq!(expected_artifact("linux", ""), "libopentui.a or libopentui.so");
        assert_eq!(expected_artifact("windows", "msvc"), "opentui.lib and opentui.dll");
        assert_eq!(expected_artifact("windows", "gnu"), "libopentui.dll.a and opentui.dll");
        assert_eq!(
            expected_artifact("freebsd", ""),
            "a supported libopentui static/shared artifact"
        );
    }

    #[test]
    fn parse_provenance_accepts_good_record() {
        let p = parse_provenance(GOOD).expect("good provenance must parse");
        assert_eq!(p.zig_commit, "0.13.0+abc123");
        assert_eq!(p.source_pin, "opentui-v1.2.3");
    }

    #[test]
    fn parse_provenance_rejects_missing_field() {
        let err = parse_provenance("zig_commit=x\nbuild_flags=y\nsource_pin=z\n").unwrap_err();
        assert!(err.contains("sha256"), "unexpected: {err}");
    }

    #[test]
    fn parse_provenance_rejects_bad_sha256() {
        let bad = GOOD.replace("0123", "ZZZZ");
        let err = parse_provenance(&bad).unwrap_err();
        assert!(err.contains("sha256"), "unexpected: {err}");
        let short = "zig_commit=x\nbuild_flags=y\nsource_pin=z\nsha256=abc\n";
        assert!(parse_provenance(short).unwrap_err().contains("sha256"));
    }

    #[test]
    fn drift_message_names_triple_and_expected_artifact() {
        let msg = drift_message(
            "aarch64-apple-darwin",
            &PathBuf::from("/x/native/lib/aarch64-apple-darwin"),
            "libopentui.a or libopentui.dylib",
            "provenance file unreadable",
        );
        assert!(msg.contains("aarch64-apple-darwin"), "{msg}");
        assert!(msg.contains("expected_artifact"), "{msg}");
        assert!(msg.contains("provenance"), "{msg}");
    }

    #[test]
    fn validate_missing_file_fails_closed_with_expected_artifact() {
        let dir = PathBuf::from("/nonexistent-triple-dir-brg-abi");
        let err = validate_provenance_file(&dir, "libopentui.a or libopentui.so").unwrap_err();
        assert!(err.contains("PROVENANCE") || err.contains("provenance"), "{err}");
        assert!(err.contains("expected_artifact"), "{err}");
        assert!(err.contains("libopentui.a or libopentui.so"), "{err}");
    }
}
