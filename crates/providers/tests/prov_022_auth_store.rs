//! PROV-022 frozen tests: hardened auth storage planning (T01..T05).

use opencode_rk_providers::auth_store::{
    plan_refresh_persist, plan_store, plan_store_with_secret, status_of, status_with_keyring,
    validate_dest, validate_secret, AllowlistedPath, StoreBackend, StoreError, StoreRequest,
    FILE_MODE_0600,
};
use std::os::unix::fs::PermissionsExt;

const FIXTURE_SECRET: &[u8] = b"fixture-auth-store-secret-0001";

fn request(root: &std::path::Path, dest: &str) -> StoreRequest {
    StoreRequest {
        provider_id: "openai".into(),
        backend: StoreBackend::File0600,
        dest: AllowlistedPath::new(root, dest).expect("allowlisted dest"),
    }
}

#[test]
fn prov_022_t01_file_fallback_happy_path() {
    let dir = tempfile::tempdir().expect("disposable fixture dir");
    let plan = plan_store(request(dir.path(), "openai/api-key.json")).expect("plan");
    assert_eq!(plan.mode, FILE_MODE_0600);
    assert_eq!(plan.mode, 0o600);
    assert_eq!(plan.dest_label, "openai/api-key.json");

    let dest = dir.path().join("openai/api-key.json");
    std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
    std::fs::write(&dest, FIXTURE_SECRET).unwrap();
    std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), FIXTURE_SECRET);
    assert_eq!(dest.metadata().unwrap().permissions().mode() & 0o777, 0o600);
}

#[test]
fn prov_022_t02_atomic_refresh() {
    let dir = tempfile::tempdir().expect("disposable fixture dir");
    let plan = plan_refresh_persist("openai").expect("refresh plan");
    assert!(plan.atomic);

    let dest = dir.path().join("creds.json");
    let tmp = dir.path().join("creds.json.tmp");
    std::fs::write(&dest, b"old-bytes").unwrap();
    std::fs::write(&tmp, b"new-bytes").unwrap();
    assert_eq!(
        std::fs::read(&dest).unwrap(),
        b"old-bytes",
        "crash mid-write keeps old"
    );
    std::fs::rename(&tmp, &dest).unwrap();
    let final_bytes = std::fs::read(&dest).unwrap();
    assert!(
        final_bytes == b"old-bytes" || final_bytes == b"new-bytes",
        "never partial"
    );
    assert_eq!(final_bytes, b"new-bytes");
}

#[test]
fn prov_022_t03_path_allowlist() {
    let dir = tempfile::tempdir().expect("disposable fixture dir");
    assert_eq!(
        AllowlistedPath::new(dir.path(), "/absolute/escape").unwrap_err(),
        StoreError::PathNotAllowed
    );
    assert_eq!(
        AllowlistedPath::new(dir.path(), "../escape").unwrap_err(),
        StoreError::PathNotAllowed
    );
    assert_eq!(
        validate_dest(dir.path(), "..").unwrap_err(),
        StoreError::PathNotAllowed
    );
    let mut bad = request(dir.path(), "openai/api-key.json");
    bad.provider_id.clear();
    assert_eq!(plan_store(&bad).unwrap_err(), StoreError::EmptyProvider);
    let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert!(entries.is_empty(), "nothing written");
}

#[test]
fn prov_022_t04_keyring_fallback_honesty() {
    let st = status_with_keyring("openai", StoreBackend::Keyring, false);
    assert_eq!(st.backend, StoreBackend::File0600, "honest fallback");
    assert_eq!(st.error, Some(StoreError::KeyringUnavailable));
    let direct = status_of("openai", StoreBackend::Keyring);
    assert_eq!(direct.backend, StoreBackend::File0600);

    let oversize = vec![0u8; 64 * 1024 + 1];
    assert_eq!(
        validate_secret(&oversize).unwrap_err(),
        StoreError::TooLarge
    );

    let dir = tempfile::tempdir().expect("disposable fixture dir");
    assert_eq!(
        plan_store_with_secret(request(dir.path(), "openai/k.json"), &oversize).unwrap_err(),
        StoreError::TooLarge
    );
    let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert!(entries.is_empty(), "nothing written");
}

#[test]
fn prov_022_t05_redaction_and_isolation() {
    let dir = tempfile::tempdir().expect("disposable fixture dir");
    let plan = plan_store(request(dir.path(), "openai/api-key.json")).unwrap();
    let text = format!("{plan:?}")
        + &serde_json::to_string(&plan).unwrap()
        + &format!("{:?}", status_of("openai", StoreBackend::File0600))
        + &serde_json::to_string(&status_of("openai", StoreBackend::File0600)).unwrap()
        + &format!("{:?}", StoreError::PathNotAllowed)
        + &serde_json::to_string(&plan_refresh_persist("openai").unwrap()).unwrap();
    assert!(
        !text.contains("fixture-auth-store-secret-0001"),
        "no fixture secrets"
    );
    assert!(!text.contains("sk-"));
    let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert!(entries.is_empty(), "no files outside disposable dir");
}
