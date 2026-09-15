use opencode_rk_server::web_host::{HostError, qualify_host};

#[test]
fn hst_t01_valid() {
    assert_eq!(qualify_host("example.com").unwrap(), "example.com");
    assert_eq!(
        qualify_host("my-host.example.com").unwrap(),
        "my-host.example.com"
    );
    assert_eq!(qualify_host("a1b").unwrap(), "a1b");
    assert_eq!(qualify_host(&"a".repeat(64)).unwrap(), "a".repeat(64));
}

#[test]
fn hst_t02_empty() {
    assert!(matches!(qualify_host(""), Err(HostError::EmptyHost)));
}

#[test]
fn hst_t03_bad_upper_space() {
    assert!(matches!(
        qualify_host("Example.com"),
        Err(HostError::BadHost)
    ));
    assert!(matches!(
        qualify_host("example com"),
        Err(HostError::BadHost)
    ));
    assert!(matches!(
        qualify_host("example.com "),
        Err(HostError::BadHost)
    ));
}

#[test]
fn hst_t04_too_short() {
    assert!(matches!(qualify_host("a"), Err(HostError::BadHost)));
    assert!(matches!(qualify_host("ab"), Err(HostError::BadHost)));
    assert!(matches!(
        qualify_host(&"a".repeat(65)),
        Err(HostError::BadHost)
    ));
}

#[test]
fn hst_t05_double_dot() {
    assert!(matches!(
        qualify_host("example..com"),
        Err(HostError::BadHost)
    ));
    assert!(matches!(
        qualify_host("-example.com"),
        Err(HostError::BadHost)
    ));
    assert!(matches!(
        qualify_host("example.com-"),
        Err(HostError::BadHost)
    ));
    assert!(matches!(
        qualify_host(".example.com"),
        Err(HostError::BadHost)
    ));
}
