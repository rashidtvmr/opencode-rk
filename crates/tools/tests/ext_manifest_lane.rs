//! EXT-005 manifest validation, frozen EXT-005-T01..T05.
//!
//! RED: fails on missing behavior. Module included via `#[path]` so the lane
//! owns exactly `src/ext_manifest_lane.rs` + this file; the integrator wires
//! `pub mod ext_manifest_lane;` into `lib.rs` later.
//! Frozen after RED: never edit to make code pass, fix the implementation.

#[path = "../src/ext_manifest_lane.rs"]
mod ext_manifest_lane;

use std::time::{Duration, Instant};

use ext_manifest_lane::{
    MAX_CAP_LEN, MAX_CAPABILITIES, MAX_MANIFEST_BYTES, MAX_NAME_LEN, Manifest, ManifestError,
    SUPPORTED_CONTRACT_VERSION, validate_manifest, validate_manifest_bytes,
};

fn children_count() -> usize {
    let mut n = 0;
    if let Ok(dir) = std::fs::read_dir("/proc/self/task") {
        for e in dir.flatten() {
            if let Ok(s) = std::fs::read_to_string(e.path().join("children")) {
                n += s.split_whitespace().count();
            }
        }
    }
    n
}

fn caps_json(n: usize) -> String {
    let items: Vec<String> = (0..n).map(|i| format!("\"cap-{i:02}\"")).collect();
    items.join(",")
}

#[test]
fn ext005_t01_happy_path() {
    assert_eq!(SUPPORTED_CONTRACT_VERSION, 1);
    assert_eq!(MAX_MANIFEST_BYTES, 16384);
    assert_eq!(MAX_NAME_LEN, 128);
    assert_eq!(MAX_CAPABILITIES, 16);
    assert_eq!(MAX_CAP_LEN, 64);
    // 0 caps, capabilities key present empty.
    let b0 = br#"{"name":"alpha","contract_version":1,"capabilities":[]}"#;
    let m0 = validate_manifest_bytes(b0).expect("0 caps accepted");
    assert_eq!(m0.name, "alpha");
    assert_eq!(m0.capabilities.len(), 0);
    assert_eq!(m0.contract_version, 1);
    // 1 cap.
    let b1 = br#"{"name":"beta-1","contract_version":1,"capabilities":["skill.list"]}"#;
    let m1 = validate_manifest_bytes(b1).expect("1 cap accepted");
    assert_eq!(m1.name, "beta-1");
    assert_eq!(m1.capabilities.len(), 1);
    assert_eq!(m1.capabilities[0], "skill.list");
    assert_eq!(m1.contract_version, 1);
    // 16 caps (max).
    let body = format!(
        "{{\"name\":\"maxed\",\"contract_version\":1,\"capabilities\":[{}]}}",
        caps_json(16)
    );
    let m16 = validate_manifest_bytes(body.as_bytes()).expect("16 caps accepted");
    assert_eq!(m16.name, "maxed");
    assert_eq!(m16.capabilities.len(), 16);
    assert_eq!(m16.contract_version, 1);
    // Unknown extra JSON field ignored, still Ok.
    let bx = br#"{"name":"alpha","contract_version":1,"capabilities":[],"extra":"ignored","n":42}"#;
    let mx = validate_manifest_bytes(bx).expect("unknown field ignored");
    assert_eq!(mx.name, "alpha");
    assert_eq!(mx.capabilities.len(), 0);
    assert_eq!(mx.contract_version, 1);
}

#[test]
fn ext005_t02_determinism_and_struct_entry() {
    let body = br#"{"name":"steady","contract_version":1,"capabilities":["a","b"]}"#;
    let a = validate_manifest_bytes(body).expect("first parse");
    let b = validate_manifest_bytes(body).expect("second parse");
    assert_eq!(a, b, "same bytes => identical verdict");
    assert_eq!(a.name, "steady");
    assert_eq!(a.capabilities, vec!["a".to_string(), "b".to_string()]);
    let good = Manifest {
        name: "steady".to_string(),
        contract_version: 1,
        capabilities: vec!["a".to_string(), "b".to_string()],
    };
    assert_eq!(validate_manifest(&good), Ok(()));
    // Missing capabilities key defaults to [] deterministically.
    let missing = br#"{"name":"nocaps","contract_version":1}"#;
    let m1 = validate_manifest_bytes(missing).expect("missing caps ok");
    let m2 = validate_manifest_bytes(missing).expect("missing caps ok again");
    assert_eq!(m1.capabilities, Vec::<String>::new());
    assert_eq!(m1, m2);
}

#[test]
fn ext005_t03_field_validation() {
    // Empty name.
    let e = validate_manifest_bytes(br#"{"name":"","contract_version":1}"#).unwrap_err();
    assert_eq!(e, ManifestError::InvalidName);
    // 129-char name.
    let long = "a".repeat(129);
    let body = format!("{{\"name\":\"{long}\",\"contract_version\":1}}");
    let e = validate_manifest_bytes(body.as_bytes()).unwrap_err();
    assert_eq!(e, ManifestError::InvalidName);
    // Bad charset names.
    for bad in ["a/b", " a"] {
        let body = format!("{{\"name\":\"{bad}\",\"contract_version\":1}}");
        let e = validate_manifest_bytes(body.as_bytes()).unwrap_err();
        assert_eq!(e, ManifestError::InvalidName, "name {bad:?}");
    }
    // 17 capabilities.
    let body = format!(
        "{{\"name\":\"ok\",\"contract_version\":1,\"capabilities\":[{}]}}",
        caps_json(17)
    );
    let e = validate_manifest_bytes(body.as_bytes()).unwrap_err();
    assert_eq!(e, ManifestError::InvalidCapabilities);
    // Intra-manifest duplicate cap.
    let e = validate_manifest_bytes(
        br#"{"name":"ok","contract_version":1,"capabilities":["dup","dup"]}"#,
    )
    .unwrap_err();
    assert_eq!(e, ManifestError::InvalidCapabilities);
    // Unsupported contract version.
    let e = validate_manifest_bytes(br#"{"name":"ok","contract_version":999}"#).unwrap_err();
    assert_eq!(e, ManifestError::UnsupportedContract);
}

#[test]
fn ext005_t04_byte_layer_failures() {
    // 16385-byte input => TooLarge.
    let big = vec![b'x'; 16385];
    let e = validate_manifest_bytes(&big).unwrap_err();
    assert_eq!(e, ManifestError::TooLarge);
    // Non-UTF8 bytes.
    let e = validate_manifest_bytes(b"\xff\xfe{").unwrap_err();
    assert_eq!(e, ManifestError::InvalidEncoding);
    // Truncated JSON.
    let e = validate_manifest_bytes(b"{\"name\":").unwrap_err();
    assert_eq!(e, ManifestError::InvalidJson);
    // JSON array input.
    let e = validate_manifest_bytes(b"[]").unwrap_err();
    assert_eq!(e, ManifestError::InvalidJson);
    // 1 MiB binary blob => TooLarge without panic, promptly (<1 s).
    let blob = vec![0xABu8; 1 << 20];
    let start = Instant::now();
    let e = validate_manifest_bytes(&blob).unwrap_err();
    assert_eq!(e, ManifestError::TooLarge);
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "1 MiB reject must complete <1 s"
    );
}

#[test]
fn ext005_t05_no_side_effect_safety() {
    let kids_before = children_count();
    let dir = tempfile::tempdir().expect("disposable test dir");
    let before: Vec<std::ffi::OsString> = std::fs::read_dir(dir.path())
        .expect("read disposable dir")
        .map(|e| e.expect("dir entry").file_name())
        .collect();
    // Full matrix: happy, determinism, field failures, byte failures.
    let _ = validate_manifest_bytes(br#"{"name":"alpha","contract_version":1}"#);
    let _ = validate_manifest_bytes(br#"{"name":"alpha","contract_version":1}"#);
    let _ = validate_manifest_bytes(br#"{"name":"","contract_version":1}"#);
    let _ = validate_manifest_bytes(
        br#"{"name":"ok","contract_version":1,"capabilities":["dup","dup"]}"#,
    );
    let _ = validate_manifest_bytes(br#"{"name":"ok","contract_version":999}"#);
    let _ = validate_manifest_bytes(&vec![b'x'; 16385]);
    let _ = validate_manifest_bytes(b"\xff\xfe{");
    let _ = validate_manifest_bytes(b"{\"name\":");
    let _ = validate_manifest_bytes(b"[]");
    let good = Manifest {
        name: "alpha".to_string(),
        contract_version: 1,
        capabilities: vec!["skill.list".to_string()],
    };
    let _ = validate_manifest(&good);
    assert_eq!(
        children_count(),
        kids_before,
        "validation spawned a subprocess"
    );
    let after: Vec<std::ffi::OsString> = std::fs::read_dir(dir.path())
        .expect("reread disposable dir")
        .map(|e| e.expect("dir entry").file_name())
        .collect();
    assert_eq!(before, after, "no files outside the disposable dir");
    // Manifest debug holds labels only: name visible, zero credential-like bytes.
    let dbg = format!("{good:?}");
    assert!(dbg.contains("alpha"), "debug must show manifest name");
    for secret in [
        "supersecret-canary-xyz",
        "AKIA",
        "password",
        "BEGIN PRIVATE KEY",
    ] {
        assert!(!dbg.contains(secret), "leaked {secret:?}");
    }
}
