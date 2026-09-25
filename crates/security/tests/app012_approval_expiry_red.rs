//! APP012-APPROVAL-EXPIRY-RED: grant expiry is exclusive at the boundary.
#![forbid(unsafe_code)]

use opencode_rk_security::{
    app_policy::{
        decide, AppDecision, ExpectedScope, Grant, GrantLedger, OperationDigest, PolicyDeny,
        Scope,
    },
    FileAction, OperationIntent, PermissionBroker, SecurityPolicy,
};
use opencode_rk_contracts::ApprovalId;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

const NOW_SECS: u64 = 1_750_000_000;

fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(NOW_SECS)
}

fn intent() -> OperationIntent {
    OperationIntent::File {
        action: FileAction::Delete,
        path: PathBuf::from("/work/project/generated.tmp"),
    }
}

fn expected(session: opencode_rk_contracts::SessionId, at: SystemTime) -> ExpectedScope {
    ExpectedScope {
        workspace: PathBuf::from("/work/project"),
        session,
        requester: "local-user".to_owned(),
        now: at,
        policy_version: 1,
    }
}

fn grant(
    intent: &OperationIntent,
    session: opencode_rk_contracts::SessionId,
    expires_at: SystemTime,
) -> Grant {
    Grant::new(
        ApprovalId::new(),
        OperationDigest::of(intent),
        Scope::new("/work/project", session, "local-user", expires_at, 1).unwrap(),
        false,
    )
}

fn broker() -> PermissionBroker {
    PermissionBroker::new(SecurityPolicy::lean_default("/work/project"))
}

#[test]
fn approval_expiry_is_exclusive_and_expired_grant_has_no_side_effect() {
    let operation = intent();
    let session = opencode_rk_contracts::SessionId::new();

    let before = grant(&operation, session, now() + Duration::from_secs(1));
    let before_scope = expected(session, now());
    assert_eq!(
        before.covers(&operation, &before_scope),
        Ok(()),
        "grant before expiry must cover the operation"
    );
    let mut before_ledger = GrantLedger::new();
    assert_eq!(
        decide(
            &broker(),
            &operation,
            Some(&before),
            &before_scope,
            &mut before_ledger,
        ),
        AppDecision::Allow
    );
    assert!(before_ledger.contains(&before.approval));

    let at_boundary = grant(&operation, session, now());
    let at_scope = expected(session, now());
    assert_eq!(
        at_boundary.covers(&operation, &at_scope),
        Err(PolicyDeny::Expired),
        "now == expires_at must be typed as expired"
    );
    let mut at_ledger = GrantLedger::new();
    assert_eq!(
        decide(
            &broker(),
            &operation,
            Some(&at_boundary),
            &at_scope,
            &mut at_ledger,
        ),
        AppDecision::Deny {
            reason: PolicyDeny::Expired.to_string(),
        }
    );
    assert!(
        at_ledger.is_empty(),
        "expired approval must not consume authorization state"
    );

    let after = grant(&operation, session, now() - Duration::from_secs(1));
    assert_eq!(
        after.covers(&operation, &expected(session, now())),
        Err(PolicyDeny::Expired)
    );
}
