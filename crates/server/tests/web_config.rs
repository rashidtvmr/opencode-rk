use opencode_rk_server::web_config::{build_web_config, WebConfigError};

#[test]
fn webcfg_t01_valid() {
    let cfg = build_web_config("Web UI", 3000, false).unwrap();
    assert_eq!(cfg.title, "Web UI");
    assert_eq!(cfg.port, 3000);
    assert!(!cfg.dev);
}

#[test]
fn webcfg_t02_empty_title() {
    let r = build_web_config("", 3000, false);
    assert!(matches!(r, Err(WebConfigError::EmptyTitle)));
    let r = build_web_config("   ", 3000, false);
    assert!(matches!(r, Err(WebConfigError::EmptyTitle)));
}

#[test]
fn webcfg_t03_zero_port() {
    let r = build_web_config("Web UI", 0, false);
    assert!(matches!(r, Err(WebConfigError::ZeroPort)));
}

#[test]
fn webcfg_t04_trims_title() {
    let cfg = build_web_config("  Web UI  ", 3000, false).unwrap();
    assert_eq!(cfg.title, "Web UI");
}

#[test]
fn webcfg_t05_dev_flag_kept() {
    let cfg = build_web_config("Web UI", 3000, true).unwrap();
    assert!(cfg.dev);
}
