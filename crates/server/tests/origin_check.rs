use opencode_rk_server::origin_check::origin_allowed;

fn allow() -> Vec<String> {
    vec![
        "https://example.com".to_string(),
        "https://app.example.com".to_string(),
    ]
}

#[test]
fn org_t01_hit() {
    assert!(origin_allowed(&allow(), "https://example.com"));
}

#[test]
fn org_t02_miss() {
    assert!(!origin_allowed(&allow(), "https://evil.example.com"));
}

#[test]
fn org_t03_empty() {
    assert!(!origin_allowed(&allow(), ""));
    assert!(!origin_allowed(&allow(), "   "));
}

#[test]
fn org_t04_case_sensitive() {
    assert!(!origin_allowed(&allow(), "https://EXAMPLE.com"));
}

#[test]
fn org_t05_empty_list() {
    let empty: Vec<String> = vec![];
    assert!(!origin_allowed(&empty, "https://example.com"));
}
