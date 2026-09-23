//! PROV-023 runtime RED: runtime request path consumes only documented catalog entries.
//!
//! The frozen `prov_023_provider_catalog` suite validates the static JSON file
//! with a test-local lookup. This suite drives the real public seam
//! (`request_profile::profile_for`) and proves provider request resolution
//! honors the documented catalog: unknown providers/endpoints fail closed with
//! no guessed URL, and resolved profiles carry no secret bytes. No file I/O,
//! no network, no product edits.

use opencode_rk_providers::request_profile::{profile_for, RequestError};

/// T01: documented third catalog provider resolves through the runtime path.
///
/// `docs/provider-compatibility.json` documents `google` / `generate-content`;
/// the hardcoded runtime path knows only openai/anthropic, so this fails RED
/// with `UnknownProvider` until the runtime consumes the versioned catalog.
#[test]
fn prov_023_runtime_t01_documented_third_provider_resolves() {
    let profile = profile_for("google", "generate-content").expect("google profile resolves");
    assert!(
        profile.endpoint.starts_with("https://"),
        "catalog endpoints are https-only"
    );
    assert!(
        profile.endpoint.contains("googleapis.com"),
        "resolved endpoint from documented entry, never guessed"
    );
}

/// T02: unknown provider fails closed; no endpoint is synthesized.
#[test]
fn prov_023_runtime_t02_unknown_provider_fail_closed() {
    assert_eq!(
        profile_for("no-such-provider", "chat-completions").unwrap_err(),
        RequestError::UnknownProvider
    );
}

/// T03: unknown endpoint kind on a known provider fails closed.
#[test]
fn prov_023_runtime_t03_unknown_endpoint_fail_closed() {
    assert_eq!(
        profile_for("openai", "no-such-endpoint").unwrap_err(),
        RequestError::UndocumentedEndpoint
    );
}

/// T04: resolved profile rendering carries no secret values.
#[test]
fn prov_023_runtime_t04_no_secret_bytes() {
    let profile =
        profile_for("openai", "chat-completions").expect("documented openai profile");
    let rendered =
        format!("{profile:?}") + &serde_json::to_string(&profile).expect("profile serializes");
    assert!(!rendered.contains("sk-"), "no secret bytes");
    assert!(!rendered.contains("Bearer "), "no bearer token values");
}
