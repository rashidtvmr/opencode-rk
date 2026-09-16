use opencode_rk_providers::int_ledger::{append_key, MAX_LEDGER_KEYS};

#[test]
fn le_t01_append() {
    let mut keys = Vec::new();
    append_key(&mut keys, "alpha").expect("valid key appends");
    assert_eq!(keys, vec!["alpha".to_string()]);
}

#[test]
fn le_t02_empty() {
    use opencode_rk_providers::int_ledger::LedgerEntryError;
    let mut keys = Vec::new();
    match append_key(&mut keys, "") {
        Err(LedgerEntryError::EmptyKey) => {}
        other => panic!("expected EmptyKey, got {other:?}"),
    }
    assert!(keys.is_empty());
}

#[test]
fn le_t03_dup_ok() {
    let mut keys = Vec::new();
    append_key(&mut keys, "dup").expect("first append valid");
    append_key(&mut keys, "dup").expect("dup idempotent");
    assert_eq!(keys.len(), 1);
    assert_eq!(keys, vec!["dup".to_string()]);
}

#[test]
fn le_t04_overflow() {
    use opencode_rk_providers::int_ledger::LedgerEntryError;
    assert_eq!(MAX_LEDGER_KEYS, 256);
    let mut keys: Vec<String> = (0..MAX_LEDGER_KEYS).map(|i| format!("k{i}")).collect();
    match append_key(&mut keys, "overflow") {
        Err(LedgerEntryError::TooMany { max, actual }) => {
            assert_eq!(max, MAX_LEDGER_KEYS);
            assert_eq!(actual, MAX_LEDGER_KEYS);
        }
        other => panic!("expected TooMany, got {other:?}"),
    }
    assert_eq!(keys.len(), MAX_LEDGER_KEYS);
}

#[test]
fn le_t05_order() {
    let mut keys = Vec::new();
    for k in ["a", "b", "c"] {
        append_key(&mut keys, k).expect("valid key appends");
    }
    assert_eq!(
        keys,
        vec!["a".to_string(), "b".to_string(), "c".to_string()]
    );
}
