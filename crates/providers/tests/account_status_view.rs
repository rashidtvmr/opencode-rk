use opencode_rk_providers::account_status::{
    project_account_status, AccountStatusError, AccountStatusRecord, MAX_STATUS_ACCOUNTS,
};

fn rec(
    account_id: &str,
    provider_id: &str,
    active: bool,
    locked_until: Option<u64>,
    last_error: Option<&str>,
) -> AccountStatusRecord {
    AccountStatusRecord {
        account_id: account_id.to_owned(),
        provider_id: provider_id.to_owned(),
        active,
        locked_until,
        last_error: last_error.map(str::to_owned),
    }
}

#[test]
fn route_008_t01_active_projects_active() {
    let accounts = vec![rec("a1", "p1", true, None, None)];
    let views = project_account_status(&accounts, 100).expect("active account projects");
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].account_id, "a1");
    assert_eq!(views[0].provider_id, "p1");
    assert_eq!(views[0].state, "active");
    assert_eq!(views[0].retry_at, None);
}

#[test]
fn route_008_t02_inactive_projects_inactive() {
    // !active wins even when a future lock is present.
    let accounts = vec![rec("a1", "p1", false, Some(200), Some("boom"))];
    let views = project_account_status(&accounts, 100).expect("inactive account projects");
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].state, "inactive");
    assert_eq!(views[0].retry_at, None);
}

#[test]
fn route_008_t03_locked_projects_locked_with_retry() {
    // Overlong last_error must not panic; it is not part of the view.
    let long_error = "e".repeat(500);
    let accounts = vec![rec("a1", "p1", true, Some(200), Some(&long_error))];
    let views = project_account_status(&accounts, 100).expect("locked account projects");
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].state, "locked");
    assert_eq!(views[0].retry_at, Some(200));
}

#[test]
fn route_008_t04_expired_lock_projects_active() {
    // locked_until <= now is expired; output sorted by (provider_id, account_id).
    let accounts = vec![
        rec("b", "p2", true, None, None),
        rec("a", "p2", true, Some(100), None),
        rec("c", "p1", true, Some(50), None),
    ];
    let views = project_account_status(&accounts, 100).expect("expired locks project active");
    assert!(views.iter().all(|v| v.state == "active"));
    assert!(views.iter().all(|v| v.retry_at.is_none()));
    let keys: Vec<(&str, &str)> = views
        .iter()
        .map(|v| (v.provider_id.as_str(), v.account_id.as_str()))
        .collect();
    assert_eq!(keys, vec![("p1", "c"), ("p2", "a"), ("p2", "b")]);
}

#[test]
fn route_008_t05_empty_and_overflow_are_typed_errors() {
    let empty: Vec<AccountStatusRecord> = vec![];
    assert_eq!(
        project_account_status(&empty, 100).expect_err("empty must error"),
        AccountStatusError::EmptyAccounts
    );
    let many: Vec<AccountStatusRecord> = (0..MAX_STATUS_ACCOUNTS + 1)
        .map(|i| rec(&format!("a{i:03}"), "p1", true, None, None))
        .collect();
    assert_eq!(
        project_account_status(&many, 100).expect_err("overflow must error"),
        AccountStatusError::TooManyAccounts {
            max: MAX_STATUS_ACCOUNTS,
            actual: MAX_STATUS_ACCOUNTS + 1,
        }
    );
}
