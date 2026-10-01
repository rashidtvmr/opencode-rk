//! DISC-101: native caller discovery identity/authentication contract.
#![forbid(unsafe_code)]

#[test]
fn disc101_native_discovery_auth_contract() {
    let script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/e2e/discovery_identity.py");
    let status = std::process::Command::new("python3")
        .arg(script)
        .envs(
            ["OC2_TEST_BINARY", "NATIVE_LIBRARY", "BUILD_JSON"]
                .into_iter()
                .filter_map(|key| std::env::var_os(key).map(|value| (key, value))),
        )
        .status()
        .expect("spawn DISC-101 fixture");
    assert!(status.success(), "DISC-101 fixture failed: {status}");
}
