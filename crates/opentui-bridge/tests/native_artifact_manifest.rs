//! Controller-corrected and refrozen TUI-011 native artifact contract.
//! Std-only; no shell/network. Run:
//!   CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-opentui-bridge --test native_artifact_manifest -- --test-threads=1

#![forbid(unsafe_code)]

use std::fs;
use std::path::PathBuf;

const MAX_BYTES: u64 = 64 * 1024;
const MAX_ARTIFACT_BYTES: u64 = 128 * 1024 * 1024;
const EXPECTED_COMMIT: &str = "c01292fd0837bafd07ce458c74416b2b375a41ab";
const EXPECTED_TARGET: &str = "aarch64-apple-darwin";
const MH_MAGIC_64: u32 = 0xFEEDFACF;
const MH_CIGAM_64: u32 = 0xCFFAEDFE;
const CPU_TYPE_ARM64: i32 = 0x0100000C;

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
fn native_dir() -> PathBuf {
    crate_dir().join("native")
}

/// Extract a string value for a given key from raw JSON text.
fn extract_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{}\"", key);
    let idx = json.find(&pat)?;
    let after = &json[idx + pat.len()..];
    let colon = after.find(':')?;
    let val_start = after[colon..].find('"')? + colon + 1;
    let rest = &after[val_start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

#[test]
fn test_manifest_exists_bounded_and_records_contract() {
    let path = native_dir().join("artifacts.json");
    let data = fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "TUI-011 RED: manifest not found at {path_display}\nCause: crates/opentui-bridge/native/artifacts.json does not exist.\nRequired: A bounded (<=64KiB) JSON manifest recording the trusted fork pin, Zig version, target, artifact path, and SHA-256.\nFix: create native/artifacts.json in the opentui-bridge crate root.\nError: {e}",
            path_display = path.display(),
            e = e
        )
    });
    if data.len() as u64 > MAX_BYTES {
        panic!(
            "TUI-011 RED: manifest is {} bytes, exceeds 64 KiB",
            data.len()
        );
    }
    let text = String::from_utf8(data)
        .unwrap_or_else(|e| panic!("TUI-011 RED: manifest is not valid UTF-8: {e}"));

    let repo = extract_str(&text, "repo")
        .unwrap_or_else(|| panic!("TUI-011 RED: manifest missing fork repo URL"));
    if repo.is_empty() {
        panic!("TUI-011 RED: fork repo is empty")
    }

    let commit = extract_str(&text, "commit")
        .unwrap_or_else(|| panic!("TUI-011 RED: manifest missing fork commit"));
    if commit != EXPECTED_COMMIT {
        panic!("TUI-011 RED: fork commit mismatch: Found {commit} Required {EXPECTED_COMMIT}")
    }

    let zig_version = extract_str(&text, "zig_version")
        .unwrap_or_else(|| panic!("TUI-011 RED: manifest missing zig_version"));
    if zig_version.is_empty() {
        panic!("TUI-011 RED: zig_version is empty")
    }

    let target = extract_str(&text, "target")
        .unwrap_or_else(|| panic!("TUI-011 RED: manifest missing target"));
    if target != EXPECTED_TARGET {
        panic!("TUI-011 RED: target mismatch: Found {target} Required {EXPECTED_TARGET}")
    }

    let artifact_path = extract_str(&text, "path")
        .unwrap_or_else(|| panic!("TUI-011 RED: manifest missing artifact path"));
    if !artifact_path.ends_with("libopentui.dylib") {
        panic!("TUI-011 RED: artifact path must end with libopentui.dylib, found: {artifact_path}")
    }
    if !artifact_path.contains(EXPECTED_TARGET) {
        panic!("TUI-011 RED: artifact path must contain target {EXPECTED_TARGET}, found: {artifact_path}")
    }

    let sha = extract_str(&text, "sha256")
        .unwrap_or_else(|| panic!("TUI-011 RED: manifest missing sha256"));
    if sha.len() != 64
        || !sha
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    {
        panic!("TUI-011 RED: sha256 must be lowercase 64-hex, found: {sha}")
    }
}

#[test]
fn test_artifact_exists_and_is_macho_arm64() {
    let manifest = native_dir().join("artifacts.json");
    let text = fs::read_to_string(&manifest)
        .unwrap_or_else(|e| panic!("TUI-011 RED: cannot read manifest: {e}"));
    let artifact_rel = extract_str(&text, "path")
        .unwrap_or_else(|| panic!("TUI-011 RED: manifest missing artifact path"));
    let full_path = native_dir().join(artifact_rel);

    if !full_path.is_file() {
        panic!(
            "TUI-011 RED: artifact file not found at {}\nCause: the native artifact libopentui.dylib does not exist at the manifest-declared path.\nRequired: a Mach-O arm64 dylib at native/lib/aarch64-apple-darwin/libopentui.dylib.\nFix: build and vendor the pinned OpenTUI native artifact for aarch64-apple-darwin.",
            full_path.display()
        );
    }

    let metadata = full_path
        .metadata()
        .unwrap_or_else(|e| panic!("TUI-011 RED: cannot stat artifact: {e}"));
    if metadata.len() == 0 {
        panic!("TUI-011 RED: artifact is empty (0 bytes)");
    }
    if metadata.len() as u64 > MAX_ARTIFACT_BYTES {
        panic!(
            "TUI-011 RED: artifact is {} bytes, exceeds {} byte native artifact bound",
            metadata.len(),
            MAX_ARTIFACT_BYTES
        );
    }

    let data =
        fs::read(&full_path).unwrap_or_else(|e| panic!("TUI-011 RED: cannot read artifact: {e}"));
    if data.len() < 8 {
        panic!("TUI-011 RED: artifact is too small (<8 bytes) to be a Mach-O header");
    }

    let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    let is_macho_64 = magic == MH_MAGIC_64 || magic == MH_CIGAM_64;
    if !is_macho_64 {
        panic!("TUI-011 RED: artifact magic 0x{:08X} is not Mach-O 64-bit (expected 0x{:08X} or 0x{:08X})", magic, MH_MAGIC_64, MH_CIGAM_64);
    }

    let cpu_bytes = [data[4], data[5], data[6], data[7]];
    let cputype = if magic == MH_CIGAM_64 {
        i32::from_le_bytes(cpu_bytes)
    } else {
        i32::from_be_bytes(cpu_bytes)
    };
    if cputype != CPU_TYPE_ARM64 {
        panic!(
            "TUI-011 RED: artifact cputype 0x{:08X} is not ARM64 (0x{:08X})",
            cputype, CPU_TYPE_ARM64
        );
    }
}

#[test]
fn test_license_and_sbom_sidecars_exist_and_bounded() {
    let notices_path = native_dir().join("NOTICES");
    let sbom_path = native_dir().join("sbom.json");

    if !notices_path.is_file() {
        panic!(
            "TUI-011 RED: license sidecar NOTICES not found at {}",
            notices_path.display()
        );
    }
    let notices_meta = notices_path
        .metadata()
        .unwrap_or_else(|e| panic!("TUI-011 RED: cannot stat NOTICES: {e}"));
    if notices_meta.len() == 0 {
        panic!("TUI-011 RED: NOTICES file is empty");
    }
    if notices_meta.len() as u64 > MAX_BYTES {
        panic!(
            "TUI-011 RED: NOTICES is {} bytes, exceeds 64 KiB",
            notices_meta.len()
        );
    }

    if !sbom_path.is_file() {
        panic!(
            "TUI-011 RED: SBOM sidecar sbom.json not found at {}",
            sbom_path.display()
        );
    }
    let sbom_meta = sbom_path
        .metadata()
        .unwrap_or_else(|e| panic!("TUI-011 RED: cannot stat sbom.json: {e}"));
    if sbom_meta.len() == 0 {
        panic!("TUI-011 RED: sbom.json file is empty");
    }
    if sbom_meta.len() as u64 > MAX_BYTES {
        panic!(
            "TUI-011 RED: sbom.json is {} bytes, exceeds 64 KiB",
            sbom_meta.len()
        );
    }

    let sbom_text = fs::read_to_string(&sbom_path)
        .unwrap_or_else(|e| panic!("TUI-011 RED: sbom.json is not valid UTF-8: {e}"));
    if !sbom_text.contains(EXPECTED_COMMIT) {
        panic!("TUI-011 RED: SBOM does not contain the fork commit {EXPECTED_COMMIT}");
    }
}
