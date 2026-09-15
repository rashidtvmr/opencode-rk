use opencode_rk_sessions::share::{ShareLedger, MAX_SHARE_LINKS};

#[test]
fn share_t01_create_and_list() {
    let mut ledger = ShareLedger::new();
    ledger.create("sess-1", "tok-abc", 100).unwrap();
    ledger.create("sess-1", "tok-def", 101).unwrap();
    let list = ledger.list();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].token, "tok-abc");
    assert_eq!(list[0].session_id, "sess-1");
    assert_eq!(list[0].created_at, 100);
    assert!(!list[0].revoked);
    assert_eq!(list[1].token, "tok-def");
    assert!(ledger.create("", "tok-x", 102).is_err());
}

#[test]
fn share_t02_active_filters_revoked() {
    let mut ledger = ShareLedger::new();
    ledger.create("s1", "tok-a", 1).unwrap();
    ledger.create("s1", "tok-b", 2).unwrap();
    ledger.revoke("tok-a").unwrap();
    let active = ledger.active();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].token, "tok-b");
    assert!(
        ledger
            .list()
            .iter()
            .find(|l| l.token == "tok-a")
            .unwrap()
            .revoked
    );
}

#[test]
fn share_t03_revoke_unknown_rejected() {
    let mut ledger = ShareLedger::new();
    ledger.create("s1", "tok-a", 1).unwrap();
    let err = ledger.revoke("nope").unwrap_err();
    assert!(matches!(
        err,
        opencode_rk_sessions::share::ShareError::UnknownToken { .. }
    ));
}

#[test]
fn share_t04_bad_token_rejected() {
    let mut ledger = ShareLedger::new();
    assert!(matches!(
        ledger.create("s1", "", 1).unwrap_err(),
        opencode_rk_sessions::share::ShareError::BadToken
    ));
    assert!(matches!(
        ledger.create("s1", "has/slash", 1).unwrap_err(),
        opencode_rk_sessions::share::ShareError::BadToken
    ));
    assert!(matches!(
        ledger.create("s1", "has space", 1).unwrap_err(),
        opencode_rk_sessions::share::ShareError::BadToken
    ));
    ledger.create("s1", "dup", 1).unwrap();
    assert!(matches!(
        ledger.create("s1", "dup", 2).unwrap_err(),
        opencode_rk_sessions::share::ShareError::BadToken
    ));
}

#[test]
fn share_t05_overflow_rejected() {
    let mut ledger = ShareLedger::new();
    for i in 0..MAX_SHARE_LINKS {
        ledger
            .create("s1", &format!("tok-{i:04}"), i as u64)
            .unwrap();
    }
    let err = ledger.create("s1", "one-too-many", 999).unwrap_err();
    match err {
        opencode_rk_sessions::share::ShareError::TooManyLinks { max, actual } => {
            assert_eq!(max, MAX_SHARE_LINKS);
            assert_eq!(actual, MAX_SHARE_LINKS);
        }
        other => panic!("expected TooManyLinks, got {other:?}"),
    }
}
