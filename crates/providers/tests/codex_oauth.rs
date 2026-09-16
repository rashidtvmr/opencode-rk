//! PROV-016 frozen tests T01..T05: Codex OAuth lifecycle state machine.
//!
//! Product: `crates/providers/src/codex_oauth.rs`. Pure in-memory machine; fake
//! grant issuers only; no network, no filesystem outside disposable dir.

use opencode_rk_providers::codex_oauth::{
    begin_login, begin_login_with, complete_login, logout, refresh, route_target, status,
    CodexAuthState, CodexError, HumanGrant, RefreshGrant,
};

const EXPIRY_MS: u64 = 1_700_000_000_000;
const LATER_MS: u64 = EXPIRY_MS + 3_600_000;

// T01: consent happy path.
#[test]
fn prov_016_t01_consent_happy_path() {
    let state = begin_login();
    let (url, device) = match &state {
        CodexAuthState::PendingConsent {
            consent_url,
            device_code,
        } => (consent_url.clone(), device_code.clone()),
        other => panic!("begin_login must yield PendingConsent, got {other:?}"),
    };
    assert!(url.starts_with("https://"), "consent url is https-only");
    assert!(!device.is_empty());
    let ready = complete_login(&state, HumanGrant::for_test(EXPIRY_MS))
        .expect("explicit human grant completes login");
    assert!(matches!(ready, CodexAuthState::Ready { expires_at_ms } if expires_at_ms == EXPIRY_MS));
    let view = status(&ready);
    assert_eq!(view.provider, "openai-codex");
    assert_eq!(view.auth_mode, "official-oauth");
    assert_eq!(view.expires_at_ms, Some(EXPIRY_MS));
}

// T02: refresh extends expiry; logout clears to LoggedOut with no expiry.
#[test]
fn prov_016_t02_refresh_and_logout() {
    let pending = begin_login();
    let expired_candidate =
        complete_login(&pending, HumanGrant::for_test(EXPIRY_MS)).expect("login completes");
    let refreshed = refresh(&expired_candidate, RefreshGrant::for_test(LATER_MS))
        .expect("explicit refresh grant refreshes");
    assert!(
        matches!(refreshed, CodexAuthState::Ready { expires_at_ms } if expires_at_ms == LATER_MS)
    );
    // Expired state plus refresh grant also yields Ready with later expiry.
    let from_expired = refresh(&CodexAuthState::Expired, RefreshGrant::for_test(LATER_MS))
        .expect("expired state refreshes with explicit grant");
    assert!(
        matches!(from_expired, CodexAuthState::Ready { expires_at_ms } if expires_at_ms == LATER_MS)
    );
    let view = status(&from_expired);
    assert_eq!(view.error, None);
    let view = status(&refreshed);
    assert_eq!(view.expires_at_ms, Some(LATER_MS));
    let out = logout(&refreshed);
    assert_eq!(out, CodexAuthState::LoggedOut);
    let view = status(&out);
    assert_eq!(view.expires_at_ms, None);
    assert_eq!(route_target(), "openai-codex");
}

// T03: missing grant and logged-out refresh fail without state change.
#[test]
fn prov_016_t03_consent_required_and_logged_out_refresh() {
    let pending = begin_login();
    let err = complete_login(&pending, None::<HumanGrant>).unwrap_err();
    assert_eq!(err, CodexError::ConsentRequired);
    assert!(matches!(pending, CodexAuthState::PendingConsent { .. }));
    let logged_out = CodexAuthState::LoggedOut;
    let err = refresh(&logged_out, RefreshGrant::for_test(LATER_MS)).unwrap_err();
    assert_eq!(err, CodexError::NotLoggedIn);
    assert_eq!(logged_out, CodexAuthState::LoggedOut);
    // Non-https consent URL never emitted.
    assert_eq!(
        begin_login_with("http://example.com/auth", "device-1").unwrap_err(),
        CodexError::BadConsentUrl
    );
}

// T04: every state/status Debug + JSON contains zero token bytes.
#[test]
fn prov_016_t04_redaction() {
    let pending = begin_login();
    let ready = complete_login(&pending, HumanGrant::for_test(EXPIRY_MS)).expect("ready");
    let out = logout(&ready);
    let states = [
        CodexAuthState::LoggedOut,
        pending,
        ready,
        CodexAuthState::Expired,
        out,
    ];
    for state in &states {
        let debug = format!("{state:?}");
        let json = serde_json::to_string(state).expect("state serializes");
        let view = status(state);
        let view_json = serde_json::to_string(&view).expect("status serializes");
        let view_debug = format!("{view:?}");
        for text in [&debug, &json, &view_json, &view_debug] {
            assert!(!text.contains("sk-"), "no token bytes: {text}");
            assert!(!text.contains("refresh_token"), "no token field: {text}");
        }
    }
}

// T05: determinism + isolation (no network, disposable dir only).
#[test]
fn prov_016_t05_determinism_and_isolation() {
    let dir = tempfile::tempdir().expect("disposable fixture dir");
    let before = std::fs::read_dir(dir.path()).expect("read dir").count();
    let run = || {
        let pending = begin_login();
        let ready = complete_login(&pending, HumanGrant::for_test(EXPIRY_MS)).expect("ready");
        serde_json::to_vec(&status(&ready)).expect("serialize")
    };
    assert_eq!(run(), run(), "same grant sequence is byte-identical");
    assert_eq!(
        std::fs::read_dir(dir.path()).expect("read dir").count(),
        before
    );
}
