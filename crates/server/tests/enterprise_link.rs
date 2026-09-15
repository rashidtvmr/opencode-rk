use opencode_rk_server::enterprise_link::{build_link, EnterpriseError};

#[test]
fn ent_t01_valid() {
    let link = build_link("https://enterprise.example.com", "slot-1").unwrap();
    assert_eq!(link.host, "https://enterprise.example.com");
    assert_eq!(link.api_key_slot, "slot-1");
}

#[test]
fn ent_t02_empty_host() {
    let r = build_link("", "slot-1");
    assert!(matches!(r, Err(EnterpriseError::EmptyHost)));
}

#[test]
fn ent_t03_bad_scheme() {
    let r = build_link("http://enterprise.example.com", "slot-1");
    assert!(matches!(r, Err(EnterpriseError::BadHost)));
    let r = build_link("enterprise.example.com", "slot-1");
    assert!(matches!(r, Err(EnterpriseError::BadHost)));
}

#[test]
fn ent_t04_empty_slot() {
    let r = build_link("https://enterprise.example.com", "");
    assert!(matches!(r, Err(EnterpriseError::EmptySlot)));
}

#[test]
fn ent_t05_trailing_slash_stripped() {
    let link = build_link("https://enterprise.example.com///", "slot-1").unwrap();
    assert_eq!(link.host, "https://enterprise.example.com");
}
