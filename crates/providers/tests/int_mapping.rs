use opencode_rk_providers::int_mapping::{MAX_MAPPINGS, MappingError, lookup};

#[test]
fn map_t01_hit() {
    let out = lookup(&[("a", "1"), ("b", "2")], "b").expect("hit");
    assert_eq!(out, "2");
    // first match wins
    let out = lookup(&[("a", "1"), ("a", "2")], "a").expect("first wins");
    assert_eq!(out, "1");
}

#[test]
fn map_t02_empty_key() {
    match lookup(&[("a", "1")], "") {
        Err(MappingError::EmptyKey) => {}
        other => panic!("expected EmptyKey, got {other:?}"),
    }
}

#[test]
fn map_t03_empty_value() {
    match lookup(&[("a", "")], "a") {
        Err(MappingError::EmptyValue) => {}
        other => panic!("expected EmptyValue, got {other:?}"),
    }
    // any empty stored key -> EmptyKey
    match lookup(&[("", "1")], "a") {
        Err(MappingError::EmptyKey) => {}
        other => panic!("expected EmptyKey, got {other:?}"),
    }
}

#[test]
fn map_t04_unknown_is_emptyvalue() {
    // frozen enum lacks Unknown; unknown key maps to EmptyValue by contract
    match lookup(&[("a", "1")], "z") {
        Err(MappingError::EmptyValue) => {}
        other => panic!("expected EmptyValue, got {other:?}"),
    }
}

#[test]
fn map_t05_overflow() {
    assert_eq!(MAX_MAPPINGS, 128);
    let owned: Vec<(String, String)> =
        (0..(MAX_MAPPINGS + 1)).map(|i| (format!("k{i}"), format!("v{i}"))).collect();
    let refs: Vec<(&str, &str)> =
        owned.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    match lookup(&refs, "k0") {
        Err(MappingError::TooMany { max, actual }) => {
            assert_eq!(max, MAX_MAPPINGS);
            assert_eq!(actual, MAX_MAPPINGS + 1);
        }
        other => panic!("expected TooMany, got {other:?}"),
    }
}
