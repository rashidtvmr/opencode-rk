use opencode_rk_providers::oauth_flow::{plan_flow, OAuthFlowError, MAX_OAUTH_STEPS};

#[test]
fn oauth_t01_valid() {
    let out = plan_flow(&[
        ("auth", "https://example.com/auth"),
        ("token", "https://example.com/token"),
    ])
    .expect("valid flow");
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].name, "auth");
    assert_eq!(out[0].url, "https://example.com/auth");
    assert_eq!(out[1].name, "token");
    assert_eq!(out[1].url, "https://example.com/token");
}

#[test]
fn oauth_t02_empty_name() {
    let err = plan_flow(&[("", "https://example.com/auth")]).unwrap_err();
    assert!(matches!(err, OAuthFlowError::EmptyName));
}

#[test]
fn oauth_t03_bad_url() {
    let err = plan_flow(&[("auth", "http://example.com/auth")]).unwrap_err();
    assert!(matches!(err, OAuthFlowError::BadUrl));
}

#[test]
fn oauth_t04_empty_url() {
    let err = plan_flow(&[("auth", "")]).unwrap_err();
    assert!(matches!(err, OAuthFlowError::EmptyUrl));
}

#[test]
fn oauth_t05_overflow() {
    let pairs: Vec<(&str, &str)> = (0..MAX_OAUTH_STEPS + 1)
        .map(|_| ("s", "https://example.com/s"))
        .collect();
    let err = plan_flow(&pairs).unwrap_err();
    match err {
        OAuthFlowError::TooManySteps { max, actual } => {
            assert_eq!(max, MAX_OAUTH_STEPS);
            assert_eq!(actual, MAX_OAUTH_STEPS + 1);
        }
        other => panic!("expected TooManySteps, got {other:?}"),
    }
}
