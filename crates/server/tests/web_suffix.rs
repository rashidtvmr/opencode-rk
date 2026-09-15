use opencode_rk_server::web_suffix::host_matches_suffix;

#[test]
fn sfx_t01_exact() {
    assert!(host_matches_suffix("example.com", "example.com"));
}

#[test]
fn sfx_t02_subdomain() {
    assert!(host_matches_suffix("api.example.com", "example.com"));
}

#[test]
fn sfx_t03_nomatch() {
    assert!(!host_matches_suffix("example.org", "example.com"));
    assert!(!host_matches_suffix("badexample.com", "example.com"));
}

#[test]
fn sfx_t04_empty() {
    assert!(!host_matches_suffix("", "example.com"));
    assert!(!host_matches_suffix("example.com", ""));
    assert!(!host_matches_suffix("", ""));
    assert!(!host_matches_suffix("   ", "example.com"));
}

#[test]
fn sfx_t05_case() {
    assert!(host_matches_suffix("API.EXAMPLE.COM", "example.com"));
    assert!(host_matches_suffix("  Example.COM  ", "  EXAMPLE.com "));
}
