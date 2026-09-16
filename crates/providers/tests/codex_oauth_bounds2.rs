//! PROV-016 bounds2: BadDeviceCode / BadConsentUrl(length) / InvalidExpiry pins.
//!
//! Additive to frozen `tests/codex_oauth.rs` (never edited) and sibling
//! `tests/codex_oauth_bounds.rs` (never touched). Real impl only; no network,
//! no filesystem outside disposable dir.

use opencode_rk_providers::codex_oauth::{
    begin_login, begin_login_with, complete_login, complete_login_at, refresh, refresh_at,
    CodexAuthState, CodexError, HumanGrant, RefreshGrant, CODEX_CONSENT_URL,
    MAX_CONSENT_URL_BYTES, MAX_DEVICE_CODE_BYTES,
};

const EXPIRY_MS: u64 = 1_700_000_000_000;

// Empty device code rejected on all entry points.
#[test]
fn prov_016_c12_empty_device_code_rejected() {
    assert_eq!(
        begin_login_with(CODEX_CONSENT_URL, "").unwrap_err(),
        CodexError::BadDeviceCode
    );
    assert_eq!(
        HumanGrant::from_device_code("", EXPIRY_MS, true).unwrap_err(),
        CodexError::BadDeviceCode
    );
    assert_eq!(
        HumanGrant::for_device("", EXPIRY_MS).unwrap_err(),
        CodexError::BadDeviceCode
    );
}

// Oversize device code rejected; exactly MAX stays Ok.
#[test]
fn prov_016_c13_oversize_device_code_rejected() {
    assert_eq!(MAX_DEVICE_CODE_BYTES, 128);
    let oversize = "d".repeat(MAX_DEVICE_CODE_BYTES + 1);
    assert_eq!(
        begin_login_with(CODEX_CONSENT_URL, &oversize).unwrap_err(),
        CodexError::BadDeviceCode
    );
    assert_eq!(
        HumanGrant::for_device(&oversize, EXPIRY_MS).unwrap_err(),
        CodexError::BadDeviceCode
    );
    assert_eq!(
        HumanGrant::from_device_code(&oversize, EXPIRY_MS, true).unwrap_err(),
        CodexError::BadDeviceCode
    );
    let at_max = "d".repeat(MAX_DEVICE_CODE_BYTES);
    assert!(begin_login_with(CODEX_CONSENT_URL, &at_max).is_ok());
    assert!(HumanGrant::for_device(&at_max, EXPIRY_MS).is_ok());
}

// >2048B consent URL rejected; exactly 2048B https URL stays Ok.
#[test]
fn prov_016_c14_consent_url_length_cap() {
    assert_eq!(MAX_CONSENT_URL_BYTES, 2_048);
    let prefix = "https://x/";
    let over = format!(
        "{prefix}{}",
        "p".repeat(MAX_CONSENT_URL_BYTES + 1 - prefix.len())
    );
    assert_eq!(over.len(), MAX_CONSENT_URL_BYTES + 1);
    assert_eq!(
        begin_login_with(&over, "device-1").unwrap_err(),
        CodexError::BadConsentUrl
    );
    let at_max = format!(
        "{prefix}{}",
        "p".repeat(MAX_CONSENT_URL_BYTES - prefix.len())
    );
    assert_eq!(at_max.len(), MAX_CONSENT_URL_BYTES);
    assert!(begin_login_with(&at_max, "device-1").is_ok());
}

// Zero expiry rejected on completion and refresh.
#[test]
fn prov_016_c15_zero_expiry_rejected() {
    let pending = begin_login();
    assert_eq!(
        complete_login(&pending, HumanGrant::for_test(0)).unwrap_err(),
        CodexError::InvalidExpiry
    );
    let ready = complete_login(&pending, HumanGrant::for_test(EXPIRY_MS)).expect("ready");
    assert_eq!(
        refresh(&ready, RefreshGrant::for_test(0)).unwrap_err(),
        CodexError::InvalidExpiry
    );
    assert_eq!(
        refresh(&CodexAuthState::Expired, RefreshGrant::for_test(0)).unwrap_err(),
        CodexError::InvalidExpiry
    );
}

// Stale expiry rejected at completion boundary (expires <= now).
#[test]
fn prov_016_c16_stale_expiry_rejected_on_complete() {
    let pending = begin_login();
    assert_eq!(
        complete_login_at(&pending, HumanGrant::for_test(EXPIRY_MS), EXPIRY_MS).unwrap_err(),
        CodexError::InvalidExpiry
    );
    assert_eq!(
        complete_login_at(&pending, HumanGrant::for_test(EXPIRY_MS), EXPIRY_MS + 1).unwrap_err(),
        CodexError::InvalidExpiry
    );
    let ready =
        complete_login_at(&pending, HumanGrant::for_test(EXPIRY_MS), EXPIRY_MS - 1)
            .expect("ready");
    assert!(matches!(ready, CodexAuthState::Ready { .. }));
}

// Stale expiry rejected at refresh boundary (expires <= now).
#[test]
fn prov_016_c17_stale_expiry_rejected_on_refresh() {
    let pending = begin_login();
    let ready =
        complete_login_at(&pending, HumanGrant::for_test(EXPIRY_MS), EXPIRY_MS - 1)
            .expect("ready");
    assert_eq!(
        refresh_at(&ready, RefreshGrant::for_test(EXPIRY_MS), EXPIRY_MS).unwrap_err(),
        CodexError::InvalidExpiry
    );
    assert_eq!(
        refresh_at(
            &ready,
            RefreshGrant::for_test(EXPIRY_MS),
            EXPIRY_MS + 1
        )
        .unwrap_err(),
        CodexError::InvalidExpiry
    );
    let refreshed = refresh_at(
        &ready,
        RefreshGrant::for_test(EXPIRY_MS),
        EXPIRY_MS - 1,
    )
    .expect("refresh ok");
    assert!(matches!(refreshed, CodexAuthState::Ready { .. }));
}
