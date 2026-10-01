//! DISC-101: native caller discovery identity/authentication contract.
//!
//! Source-only test owner. Spawns the Python composite fixture which exercises
//! four cases (VALIDIDENTITY, STALEPID, LIVEFOREIGNPID, EMPTYTOKEN) through the
//! actual installed native binary. Auth controls (200/401/403) are verified via
//! the real Rust public router's Bearer middleware on the controlled endpoint.
//!
//! Attestation: OC2_TEST_BINARY, NATIVE_LIBRARY, BUILD_JSON must be absolute
//! paths to a native release build with matching SHA-256 hashes and 40-hex
//! source_sha attestation. Missing attestation = FAIL, not SKIP.
#![forbid(unsafe_code)]

#[test]
fn disc101_native_discovery_auth_contract() {
    let script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/e2e/discovery_identity.py");
    assert!(
        script.is_file(),
        "DISC-101 fixture missing: {}",
        script.display()
    );

    // Require absolute attested artifact paths — FAIL not SKIP.
    let binary = std::env::var("OC2_TEST_BINARY")
        .expect("OC2_TEST_BINARY required (absolute path to installed release binary)");
    let library = std::env::var("NATIVE_LIBRARY")
        .expect("NATIVE_LIBRARY required (absolute path to installed native library)");
    let build_json = std::env::var("BUILD_JSON")
        .expect("BUILD_JSON required (absolute path to build attestation manifest)");

    assert!(
        std::path::Path::new(&binary).is_absolute(),
        "OC2_TEST_BINARY must be absolute"
    );
    assert!(
        std::path::Path::new(&library).is_absolute(),
        "NATIVE_LIBRARY must be absolute"
    );
    assert!(
        std::path::Path::new(&build_json).is_absolute(),
        "BUILD_JSON must be absolute"
    );

    let status = std::process::Command::new("python3")
        .arg(&script)
        .env("OC2_TEST_BINARY", &binary)
        .env("NATIVE_LIBRARY", &library)
        .env("BUILD_JSON", &build_json)
        .status()
        .expect("spawn DISC-101 fixture");
    assert!(status.success(), "DISC-101 fixture failed: {status}");
}
