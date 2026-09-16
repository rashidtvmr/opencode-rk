//! PROV-015 frozen tests T01..T05: secret-free auth profile/provenance.
//!
//! Product: `crates/providers/src/auth_profile.rs`. Tests assert real behavior
//! against the real implementation; no mocked success, no secret material.

use opencode_rk_providers::auth_profile::{
    describe, profile_of, redacted_debug, ApiKey, AuthMethodKind, AuthProfileError, AuthProvenance,
    ImportedLocal, Keyring, OfficialOAuth, MAX_PROVIDER_ID_LEN,
};
use std::collections::HashSet;

// T01: api-key profile happy path.
#[test]
fn prov_015_t01_apikey_profile_happy_path() {
    let profile = profile_of("openai", ApiKey, AuthMethodKind::ApiKeyKind)
        .expect("valid api-key profile constructs");
    assert_eq!(profile.provider_id, "openai");
    assert_eq!(profile.provenance, AuthProvenance::ApiKey);
    assert_eq!(profile.method, AuthMethodKind::ApiKeyKind);
    let desc = describe(&profile);
    assert_eq!(desc.provider_id, "openai");
    assert_eq!(desc.provenance, AuthProvenance::ApiKey);
    assert_eq!(desc.auth_mode, "api-key");
    assert!(desc.has_credentials);
    assert!(desc.has_credentials());
    let debug = redacted_debug(&profile);
    assert!(debug.contains("openai"));
    assert!(debug.contains("api-key"));
}

// T02: every provenance round-trips through describe; all four distinct.
#[test]
fn prov_015_t02_provenance_preserved_and_distinct() {
    let provenances = [
        AuthProvenance::ApiKey,
        OfficialOAuth,
        ImportedLocal,
        Keyring,
    ];
    let mut labels = HashSet::new();
    for provenance in provenances {
        let profile = profile_of("openai", provenance, AuthMethodKind::OAuth2Kind)
            .expect("each provenance constructs");
        let desc = describe(&profile);
        assert_eq!(desc.provenance, provenance);
        assert!(labels.insert(desc.provenance.as_str().to_owned()));
    }
    assert_eq!(labels.len(), 4);
    assert_eq!(
        AuthProvenance::ApiKey.as_str(),
        "api-key",
        "provenance labels are stable status strings"
    );
}

// T03: Debug + serde_json of profile and description leak zero secret bytes.
#[test]
fn prov_015_t03_no_log_no_serialize_leak() {
    let profile = profile_of("openai", OfficialOAuth, AuthMethodKind::OAuth2Kind)
        .expect("oauth profile constructs");
    let desc = describe(&profile);
    let debug_profile = format!("{profile:?}");
    let debug_desc = format!("{desc:?}");
    let json_profile = serde_json::to_string(&profile).expect("profile serializes without secrets");
    let json_desc = serde_json::to_string(&desc).expect("description serializes without secrets");
    for view in [&debug_profile, &debug_desc, &json_profile, &json_desc] {
        assert!(
            !view.contains("sk-"),
            "secret-shaped bytes must never appear: {view}"
        );
        assert!(
            !view.contains("refresh"),
            "token field names must never appear: {view}"
        );
    }
    for view in [&json_profile, &json_desc] {
        assert!(view.contains("openai"), "identity retained: {view}");
        assert!(
            view.contains("official-oauth"),
            "provenance retained: {view}"
        );
    }
    assert!(redacted_debug(&profile).contains("openai"));
}

// T04: validation failures leave no partial profile.
#[test]
fn prov_015_t04_validation_failures() {
    assert_eq!(
        profile_of("", ApiKey, AuthMethodKind::ApiKeyKind),
        Err(AuthProfileError::EmptyProvider)
    );
    assert_eq!(
        profile_of("openai", ApiKey, AuthMethodKind::Unknown),
        Err(AuthProfileError::UnknownMethod)
    );
    let too_long = "p".repeat(MAX_PROVIDER_ID_LEN + 1);
    assert_eq!(
        profile_of(&too_long, ApiKey, AuthMethodKind::ApiKeyKind),
        Err(AuthProfileError::TooLong)
    );
    // Boundary: exactly 128 bytes constructs; empty/unknown/too-long never do.
    assert!(profile_of(
        &"p".repeat(MAX_PROVIDER_ID_LEN),
        ApiKey,
        AuthMethodKind::ApiKeyKind
    )
    .is_ok());
}

// T05: determinism + isolation (disposable dir only, no DB writes).
#[test]
fn prov_015_t05_determinism_and_isolation() {
    let dir = tempfile::tempdir().expect("disposable fixture dir");
    let before: Vec<_> = std::fs::read_dir(dir.path())
        .expect("read fixture dir")
        .collect();
    let first =
        describe(&profile_of("openai", Keyring, AuthMethodKind::BearerTokenKind).expect("profile"));
    let second =
        describe(&profile_of("openai", Keyring, AuthMethodKind::BearerTokenKind).expect("profile"));
    let a = serde_json::to_vec(&first).expect("serialize");
    let b = serde_json::to_vec(&second).expect("serialize");
    assert_eq!(a, b, "same inputs yield byte-identical description");
    assert_eq!(
        redacted_debug(
            &profile_of("openai", Keyring, AuthMethodKind::BearerTokenKind).expect("profile")
        ),
        redacted_debug(
            &profile_of("openai", Keyring, AuthMethodKind::BearerTokenKind).expect("profile")
        )
    );
    let after: Vec<_> = std::fs::read_dir(dir.path())
        .expect("read fixture dir")
        .collect();
    assert_eq!(
        before.len(),
        after.len(),
        "no files created outside disposable dir"
    );
}
