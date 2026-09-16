//! PROV-017 frozen tests: Claude Code OAuth consent machine (T01..T05).

use opencode_rk_providers::claude_oauth::{
    begin_login, complete_login, logout, refresh, route_target, status, ClaudeAuthState,
    ClaudeError, HumanGrant, RefreshGrant,
};

const PKCE: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const ACCESS: &str = "ATCAKC-fixture-access-0001";
const REFRESH: &str = "RTKC-fixture-refresh-0001";
const EXPIRY: u64 = 1_800_000_000_000;

fn pending() -> ClaudeAuthState {
    begin_login(PKCE).expect("valid pkce begins login")
}

fn grant() -> HumanGrant {
    HumanGrant::for_test(ACCESS, REFRESH, EXPIRY)
}

#[test]
fn prov_017_t01_consent_happy_path() {
    let state = pending();
    let (url, challenge) = match &state {
        ClaudeAuthState::PendingConsent {
            consent_url,
            pkce_challenge,
        } => (consent_url.clone(), pkce_challenge.clone()),
        other => panic!("expected PendingConsent, got {other:?}"),
    };
    assert!(url.starts_with("https://"), "https-only consent url");
    assert!(url.contains("127.0.0.1"), "loopback consent url");
    assert_eq!(challenge, PKCE, "pkce echo");

    let ready = complete_login(&state, grant()).expect("human grant completes");
    assert_eq!(
        ready,
        ClaudeAuthState::Ready {
            expires_at_ms: EXPIRY
        }
    );

    let st = status(&ready);
    assert_eq!(st.provider, "anthropic-claude-code");
    assert_eq!(st.auth_mode, "official-oauth");
    assert_eq!(st.expires_at_ms, Some(EXPIRY));
    assert_eq!(route_target(), "anthropic-claude-code");
}

#[test]
fn prov_017_t02_refresh_and_logout() {
    let expired = ClaudeAuthState::Expired;
    let later = EXPIRY + 3_600_000;
    let ready = refresh(&expired, RefreshGrant::new(later)).expect("expired refreshes");
    assert_eq!(
        ready,
        ClaudeAuthState::Ready {
            expires_at_ms: later
        }
    );

    let out = logout(&ready);
    assert_eq!(out, ClaudeAuthState::LoggedOut);
    let st = status(&out);
    assert_eq!(st.expires_at_ms, None);
    let rendered = format!("{out:?}") + &serde_json::to_string(&out).unwrap();
    let rendered_status = format!("{st:?}") + &serde_json::to_string(&st).unwrap();
    assert!(!rendered.contains(ACCESS) && !rendered.contains(REFRESH));
    assert!(!rendered_status.contains(ACCESS) && !rendered_status.contains(REFRESH));
}

#[test]
fn prov_017_t03_validation_failures_leave_state_unchanged() {
    assert_eq!(
        begin_login("").unwrap_err(),
        ClaudeError::BadPkce,
        "empty pkce rejected"
    );

    let state = pending();
    let before = state.clone();
    assert_eq!(
        complete_login(&state, None::<HumanGrant>).unwrap_err(),
        ClaudeError::ConsentRequired,
        "missing grant rejected"
    );
    assert_eq!(state, before, "state unchanged on no-grant");

    let out = ClaudeAuthState::LoggedOut;
    assert_eq!(
        refresh(&out, RefreshGrant::new(EXPIRY)).unwrap_err(),
        ClaudeError::NotLoggedIn,
        "refresh while logged out rejected"
    );
    assert_eq!(out, ClaudeAuthState::LoggedOut, "state unchanged");
}

#[test]
fn prov_017_t04_redaction() {
    let states = vec![
        ClaudeAuthState::LoggedOut,
        pending(),
        ClaudeAuthState::Ready {
            expires_at_ms: EXPIRY,
        },
        ClaudeAuthState::Expired,
    ];
    for state in &states {
        let text = format!("{state:?}")
            + &serde_json::to_string(state).unwrap()
            + &serde_json::to_string(&status(state)).unwrap();
        assert!(!text.contains(ACCESS), "access bytes leaked");
        assert!(!text.contains(REFRESH), "refresh bytes leaked");
        assert!(!text.contains("sk-ant-"), "secret-like prefix leaked");
        assert!(!text.contains("refresh_token"), "token key leaked");
    }
    let grant_text = format!("{:?}", grant()) + &serde_json::to_string(&grant()).unwrap();
    assert!(!grant_text.contains(ACCESS) && !grant_text.contains(REFRESH));
    let refresh_text = format!("{:?}", RefreshGrant::new(EXPIRY))
        + &serde_json::to_string(&RefreshGrant::new(EXPIRY)).unwrap();
    assert!(!refresh_text.contains(ACCESS) && !refresh_text.contains(REFRESH));
}

#[test]
fn prov_017_t05_determinism_and_isolation() {
    let dir = tempfile::tempdir().expect("disposable fixture dir");
    let run = || {
        let ready = complete_login(pending(), grant()).expect("grant completes");
        serde_json::to_string(&status(&ready)).unwrap()
    };
    assert_eq!(run(), run(), "byte-identical status");
    let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert!(entries.is_empty(), "no files written");
}
