use opencode_rk_server::app_client::{build_app_client_config, AppClientError};

#[test]
fn app_t01_valid_config() {
    let cfg = build_app_client_config("https://example.com/", "/api", 5000).unwrap();
    assert_eq!(cfg.origin, "https://example.com");
    assert_eq!(cfg.api_path, "/api");
    assert_eq!(cfg.timeout_ms, 5000);
}

#[test]
fn app_t02_empty_origin_rejected() {
    let r = build_app_client_config("", "/api", 1000);
    assert!(matches!(r, Err(AppClientError::EmptyOrigin)));
}

#[test]
fn app_t03_bad_scheme_rejected() {
    let r = build_app_client_config("ftp://example.com", "/api", 1000);
    assert!(matches!(r, Err(AppClientError::BadOrigin)));
    let r = build_app_client_config("example.com", "/api", 1000);
    assert!(matches!(r, Err(AppClientError::BadOrigin)));
}

#[test]
fn app_t04_path_must_be_absolute() {
    let r = build_app_client_config("https://example.com", "api", 1000);
    assert!(matches!(r, Err(AppClientError::EmptyPath)));
    let r = build_app_client_config("https://example.com", "", 1000);
    assert!(matches!(r, Err(AppClientError::EmptyPath)));
}

#[test]
fn app_t05_zero_timeout_rejected() {
    let r = build_app_client_config("https://example.com", "/api", 0);
    assert!(matches!(r, Err(AppClientError::ZeroTimeout)));
}
