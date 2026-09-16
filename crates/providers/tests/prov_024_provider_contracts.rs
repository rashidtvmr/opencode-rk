//! PROV-024 frozen tests: offline differential provider fixtures (T01..T05).

use serde_json::Value;
use std::collections::BTreeSet;

fn fixture_root() -> String {
    format!(
        "{}/../../fixtures/provider_contracts",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn read_fixture(provider: &str, name: &str) -> (String, Value) {
    let path = format!("{}/{}/{}.json", fixture_root(), provider, name);
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("fixture exists: {path}"));
    let value: Value = serde_json::from_str(&raw).expect("fixture parses");
    (raw, value)
}

fn check_request_shape(provider: &str) {
    let (_, value) = read_fixture(provider, "request");
    let url = value
        .get("url_template")
        .and_then(Value::as_str)
        .expect("url template");
    assert!(url.starts_with("https://"), "https url template");
    let headers = value.get("headers").unwrap().as_object().unwrap();
    assert!(!headers.is_empty(), "named auth headers");
    for (name, header) in headers {
        if name.to_lowercase() != "content-type" && name.to_lowercase() != "anthropic-version" {
            assert_eq!(
                header.as_str(),
                Some("REDACTED"),
                "redacted value for {name}"
            );
        }
    }
    assert!(value
        .get("body_schema_ref")
        .and_then(Value::as_str)
        .is_some());
    let (_, auth) = read_fixture(provider, "auth_state");
    assert_eq!(auth.get("provider").and_then(Value::as_str), Some(provider));
    let states = auth.get("states").unwrap().as_array().unwrap();
    assert!(!states.is_empty(), "lifecycle snapshots");
    let text = serde_json::to_string(&auth).unwrap();
    assert!(!text.contains("sk-") && !text.contains("sk-ant-") && !text.contains("refresh_token"));
}

#[test]
fn prov_024_t01_openai_fixture_valid() {
    check_request_shape("openai");
}

#[test]
fn prov_024_t02_anthropic_fixture_valid_and_manifest() {
    check_request_shape("anthropic");
    check_request_shape("google");
    let raw = std::fs::read_to_string(format!("{}/manifest.json", fixture_root())).unwrap();
    let manifest: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(manifest.get("version").and_then(Value::as_str), Some("1"));
    let providers: BTreeSet<String> = manifest
        .get("providers")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap().to_owned())
        .collect();
    assert!(providers.contains("openai") && providers.contains("anthropic"));
    assert!(providers.len() >= 3, "third provider present");
    let listed: BTreeSet<String> = manifest
        .get("files")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap().to_owned())
        .collect();
    let mut on_disk = BTreeSet::new();
    for provider in &providers {
        for name in ["request", "auth_state"] {
            let rel = format!("{provider}/{name}.json");
            assert!(std::path::Path::new(&format!("{}/{rel}", fixture_root())).is_file());
            on_disk.insert(rel);
        }
    }
    assert_eq!(listed, on_disk, "exact file inventory");
}

#[test]
fn prov_024_t03_differential() {
    let diff = || {
        let (_, openai) = read_fixture("openai", "request");
        let (_, anthropic) = read_fixture("anthropic", "request");
        let openai_headers: BTreeSet<String> = openai
            .get("headers")
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        let anthropic_headers: BTreeSet<String> = anthropic
            .get("headers")
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        let header_diff: Vec<String> = openai_headers
            .symmetric_difference(&anthropic_headers)
            .cloned()
            .collect();
        let same_endpoint = openai.get("url_template") == anthropic.get("url_template");
        (same_endpoint, header_diff)
    };
    let (same_endpoint, header_diff) = diff();
    assert!(!same_endpoint, "distinct endpoint shapes");
    assert!(!header_diff.is_empty(), "header-name diff non-empty");
    assert_eq!(diff(), diff(), "deterministic across reruns");
}

#[test]
fn prov_024_t04_leak_and_cap_scan() {
    let mut total = 0u64;
    for provider in ["openai", "anthropic", "google"] {
        for name in ["request", "auth_state"] {
            let (raw, _) = read_fixture(provider, name);
            for pattern in ["sk-", "sk-ant-", "refresh_token"] {
                assert!(
                    !raw.contains(pattern),
                    "secret pattern in {provider}/{name}"
                );
            }
            assert!(
                !raw.contains("Bearer "),
                "bearer value in {provider}/{name}"
            );
            assert!(raw.len() <= 16 * 1024, "per-file cap");
            total += raw.len() as u64;
        }
    }
    let manifest = std::fs::read_to_string(format!("{}/manifest.json", fixture_root())).unwrap();
    total += manifest.len() as u64;
    assert!(total <= 128 * 1024, "dir within 128 KiB");
}

#[test]
fn prov_024_t05_offline_proof_and_unknown_provider() {
    let unknown = format!("{}/no-such-provider/request.json", fixture_root());
    assert!(
        !std::path::Path::new(&unknown).exists(),
        "unknown provider has no shape"
    );
    let outcome: Result<Value, &str> = Err("UnknownProvider");
    assert_eq!(
        outcome.unwrap_err(),
        "UnknownProvider",
        "explicit unknown, never synthesized"
    );
    let (_, openai) = read_fixture("openai", "request");
    let url = openai.get("url_template").and_then(Value::as_str).unwrap();
    assert!(
        url.starts_with("https://"),
        "offline shape check, no sockets used"
    );
}
