//! PROV-020 frozen tests: auth connector commands and status (T01..T05).

use opencode_rk_providers::auth_commands::{
    dispatch, inspect, list, AuthCommand, AuthCommandError, AuthGrant, AuthState,
};

const NOW: u64 = 1_000;
const EXPIRY: u64 = 4_600_000;

fn grant(provider: &str) -> AuthGrant {
    AuthGrant::oauth(provider, "acct-alice", EXPIRY)
}

#[test]
fn prov_020_t01_connect_plus_login_flow() {
    let mut state = AuthState::new();
    state.set_now_ms(NOW);
    let len_before = state.len();

    let connect = dispatch(
        AuthCommand::Connect {
            provider: "openai".into(),
        },
        &mut state,
    )
    .expect("connect");
    assert!(!connect.needs_consent);

    let login = dispatch(
        AuthCommand::Login {
            provider: "openai".into(),
        },
        &mut state,
    )
    .expect("login plans");
    assert!(login.needs_consent, "login is consent-gated");
    assert_eq!(state.len(), len_before + 1, "login adds no new record");

    let completed = state.apply_grant(grant("openai")).expect("grant applies");
    assert_eq!(completed.provider, "openai");
    assert_eq!(completed.expires_at_ms, Some(EXPIRY));
    assert!(completed.auth_mode.contains("oauth"));
}

#[test]
fn prov_020_t02_list_and_inspect() {
    let mut state = AuthState::new();
    state.set_now_ms(NOW);
    state.apply_grant(grant("openai")).unwrap();
    state.apply_grant(grant("anthropic")).unwrap();

    let listed = list(&state);
    assert_eq!(listed.statuses.len(), 2);
    assert_eq!(listed.statuses[0].provider, "anthropic");
    assert_eq!(listed.statuses[1].provider, "openai");
    assert!(!listed.truncated);
    for entry in &listed.statuses {
        assert_eq!(inspect(&state, &entry.provider).unwrap(), *entry);
    }
    assert_eq!(
        inspect(&state, "unknown-provider-xyz").unwrap_err(),
        AuthCommandError::UnknownProvider
    );
}

#[test]
fn prov_020_t03_refresh_and_logout() {
    let mut state = AuthState::new();
    state.set_now_ms(NOW);
    state.apply_grant(grant("openai")).unwrap();

    let later = EXPIRY + 3_600_000;
    state.set_refresh_expiry("openai", later).unwrap();
    let refreshed = dispatch(
        AuthCommand::Refresh {
            provider: "openai".into(),
        },
        &mut state,
    )
    .expect("refresh");
    assert_eq!(refreshed.status.expires_at_ms, Some(later));

    let logged_out = dispatch(
        AuthCommand::Logout {
            provider: "openai".into(),
        },
        &mut state,
    )
    .expect("logout");
    assert_eq!(logged_out.status.expires_at_ms, None);
    assert!(!logged_out.status.is_logged_in());
    assert_eq!(
        dispatch(
            AuthCommand::Refresh {
                provider: "openai".into()
            },
            &mut state
        )
        .unwrap_err(),
        AuthCommandError::NotLoggedIn
    );
}

#[test]
fn prov_020_t04_redaction() {
    let mut state = AuthState::new();
    state.set_now_ms(NOW);
    let effect = dispatch(
        AuthCommand::Connect {
            provider: "openai".into(),
        },
        &mut state,
    )
    .unwrap();
    let import = AuthCommand::Import {
        provider: "openai".into(),
        source: "fixture-secret-source-path".into(),
    };
    let text = format!("{effect:?}")
        + &serde_json::to_string(&effect).unwrap()
        + &format!("{import:?}")
        + &serde_json::to_string(&import).unwrap()
        + &serde_json::to_string(&list(&state)).unwrap();
    assert!(!text.contains("sk-"), "no secret substrings");
    assert!(
        !text.contains("fixture-secret-source-path"),
        "import source redacted"
    );
    assert_eq!(
        dispatch(
            AuthCommand::Inspect {
                provider: "".into()
            },
            &mut state
        )
        .unwrap_err(),
        AuthCommandError::EmptyProvider
    );
}

#[test]
fn prov_020_t05_bounds_and_isolation() {
    let dir = tempfile::tempdir().expect("disposable fixture dir");
    let mut state = AuthState::new();
    state.set_now_ms(NOW);
    for i in 0..40 {
        state.apply_grant(grant(&format!("prov-{i:02}"))).unwrap();
    }
    let listed = list(&state);
    assert_eq!(listed.statuses.len(), 32, "list capped at 32");
    assert!(listed.truncated, "truncation flagged");

    let a = serde_json::to_string(&list(&state)).unwrap();
    let b = serde_json::to_string(&list(&state)).unwrap();
    assert_eq!(a, b, "byte-identical sequence");
    let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert!(entries.is_empty(), "no files outside disposable dir");
}
