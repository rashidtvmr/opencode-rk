use opencode_rk_providers::account_sync::{
    AccountSyncError, SyncedAccount, MAX_SYNC_ACCOUNTS, plan_account_sync,
};

fn acct(id: &str, provider: &str, active: bool, updated_at: u64) -> SyncedAccount {
    SyncedAccount {
        account_id: id.to_owned(),
        provider_id: provider.to_owned(),
        active,
        updated_at,
    }
}

#[test]
fn route_009_t01_identical_is_empty_plan() {
    let snapshot = vec![
        acct("alpha", "p1", true, 10),
        acct("beta", "p1", false, 20),
    ];
    let plan = plan_account_sync(&snapshot, &snapshot).expect("identical snapshots must plan");
    assert!(plan.upsert.is_empty());
    assert!(plan.delete_ids.is_empty());
}

#[test]
fn route_009_t02_new_remote_is_upsert() {
    let cached = vec![acct("a", "p1", true, 1)];
    let remote = vec![
        acct("c", "p1", true, 3),
        acct("b", "p1", true, 2),
        acct("a", "p1", true, 1),
    ];
    let plan = plan_account_sync(&cached, &remote).expect("new remote entries must plan");
    let ids: Vec<&str> = plan
        .upsert
        .iter()
        .map(|entry| entry.account_id.as_str())
        .collect();
    assert_eq!(ids, vec!["b", "c"]);
    assert!(plan.delete_ids.is_empty());
}

#[test]
fn route_009_t03_changed_fields_are_upsert() {
    for (cached, remote) in [
        (acct("a", "p1", true, 1), acct("a", "p2", true, 1)),
        (acct("a", "p1", true, 1), acct("a", "p1", false, 1)),
        (acct("a", "p1", true, 1), acct("a", "p1", true, 2)),
    ] {
        let plan =
            plan_account_sync(std::slice::from_ref(&cached), std::slice::from_ref(&remote))
                .expect("changed field must plan");
        assert_eq!(plan.upsert, vec![remote]);
        assert!(plan.delete_ids.is_empty());
    }
}

#[test]
fn route_009_t04_missing_remote_is_delete() {
    let cached = vec![
        acct("c", "p1", true, 3),
        acct("a", "p1", true, 1),
        acct("b", "p1", true, 2),
    ];
    let remote = vec![acct("b", "p1", true, 2)];
    let plan = plan_account_sync(&cached, &remote).expect("missing remote must plan");
    assert!(plan.upsert.is_empty());
    assert_eq!(plan.delete_ids, vec!["a".to_owned(), "c".to_owned()]);
}

#[test]
fn route_009_t05_dup_and_overflow_are_typed_errors() {
    let dup_cached = vec![acct("a", "p1", true, 1), acct("a", "p1", true, 1)];
    let remote = vec![acct("a", "p1", true, 1)];
    assert_eq!(
        plan_account_sync(&dup_cached, &remote).expect_err("cached dup must fail"),
        AccountSyncError::DuplicateAccountId {
            account_id: "a".to_owned()
        }
    );

    let cached = vec![acct("a", "p1", true, 1)];
    let dup_remote = vec![acct("b", "p1", true, 1), acct("b", "p1", true, 2)];
    assert_eq!(
        plan_account_sync(&cached, &dup_remote).expect_err("remote dup must fail"),
        AccountSyncError::DuplicateAccountId {
            account_id: "b".to_owned()
        }
    );

    let big: Vec<SyncedAccount> = (0..MAX_SYNC_ACCOUNTS + 1)
        .map(|i| acct(&format!("id-{i:04}"), "p1", true, i as u64))
        .collect();
    assert_eq!(
        plan_account_sync(&big, &[]).expect_err("cached overflow must fail"),
        AccountSyncError::TooManyAccounts {
            max: MAX_SYNC_ACCOUNTS,
            actual: MAX_SYNC_ACCOUNTS + 1
        }
    );
    assert_eq!(
        plan_account_sync(&[], &big).expect_err("remote overflow must fail"),
        AccountSyncError::TooManyAccounts {
            max: MAX_SYNC_ACCOUNTS,
            actual: MAX_SYNC_ACCOUNTS + 1
        }
    );
}
