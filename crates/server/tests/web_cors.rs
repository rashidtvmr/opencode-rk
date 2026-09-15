use opencode_rk_server::web_cors::{add_origin, CorsError, MAX_CORS_ORIGINS};

#[test]
fn cors_t01_add() {
    let mut origins = Vec::new();
    add_origin(&mut origins, "https://example.com").unwrap();
    assert_eq!(origins, vec!["https://example.com".to_string()]);
}

#[test]
fn cors_t02_empty() {
    let mut origins = Vec::new();
    assert!(matches!(
        add_origin(&mut origins, ""),
        Err(CorsError::EmptyOrigin)
    ));
    assert!(matches!(
        add_origin(&mut origins, "   "),
        Err(CorsError::EmptyOrigin)
    ));
    assert!(origins.is_empty());
}

#[test]
fn cors_t03_bad_scheme() {
    let mut origins = Vec::new();
    assert!(matches!(
        add_origin(&mut origins, "ftp://example.com"),
        Err(CorsError::BadOrigin)
    ));
    assert!(matches!(
        add_origin(&mut origins, "example.com"),
        Err(CorsError::BadOrigin)
    ));
    assert!(matches!(
        add_origin(&mut origins, "http://localhost"),
        Err(CorsError::BadOrigin)
    ));
    assert!(origins.is_empty());
}

#[test]
fn cors_t04_dup_ok() {
    let mut origins = Vec::new();
    add_origin(&mut origins, "https://example.com").unwrap();
    add_origin(&mut origins, "https://example.com").unwrap();
    assert_eq!(origins.len(), 1);
}

#[test]
fn cors_t05_overflow() {
    let mut origins: Vec<String> = (0..MAX_CORS_ORIGINS)
        .map(|i| format!("https://host{i}.example.com"))
        .collect();
    assert_eq!(origins.len(), MAX_CORS_ORIGINS);
    match add_origin(&mut origins, "https://overflow.example.com") {
        Err(CorsError::TooMany { max, actual }) => {
            assert_eq!(max, MAX_CORS_ORIGINS);
            assert_eq!(actual, MAX_CORS_ORIGINS);
        }
        other => panic!("expected TooMany, got {:?}", other),
    }
    assert_eq!(origins.len(), MAX_CORS_ORIGINS);
}
