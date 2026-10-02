//! A generation number must not forge approval for an explicit `Ask` rule.
#![forbid(unsafe_code)]

use opencode_rk_security::{
    Decision, FileAction, OperationIntent, PermissionBroker, PermissionRule, PermissionSet,
    RuleEffect, SecurityPolicy,
};
use tempfile::tempdir;

#[test]
fn matching_generation_cannot_bypass_ask_on_allowed_root_write() {
    let root = tempdir().expect("disposable approval fixture");
    let path = root.path().join("generated.txt");
    let intent = OperationIntent::File {
        action: FileAction::Write,
        path,
    };
    let ask = PermissionRule::new(
        format!("{}**", root.path().to_string_lossy()),
        RuleEffect::Ask,
    );
    let broker = PermissionBroker::new(SecurityPolicy::lean_default(root.path()))
        .with_permissions(PermissionSet::new(vec![ask]));

    assert!(matches!(
        broker.authorize(&intent),
        Decision::RequireHuman { .. }
    ));
    assert!(matches!(
        broker.authorize_human_approved(&intent, broker.generation()),
        Decision::RequireHuman { .. } | Decision::Deny { .. }
    ));
}

#[test]
fn trusted_opaque_approval_allows_only_the_exact_live_call() {
    let root = tempdir().expect("disposable approval fixture");
    let intent = OperationIntent::File {
        action: FileAction::Write,
        path: root.path().join("generated.txt"),
    };
    let broker = PermissionBroker::new(SecurityPolicy::lean_default(root.path())).with_permissions(
        PermissionSet::new(vec![PermissionRule::new(
            format!("{}**", root.path().to_string_lossy()),
            RuleEffect::Ask,
        )]),
    );
    let approval = broker
        .approve_human(&intent, "session".to_owned(), 7, "call".to_owned())
        .expect("trusted broker can mint an opaque approval");
    assert_eq!(
        broker.authorize_with_human_approval(&intent, approval, "session", 7, "call"),
        Decision::Allow
    );

    let second = broker
        .approve_human(&intent, "session".to_owned(), 7, "call-2".to_owned())
        .expect("second approval is independently scoped");
    assert!(matches!(
        broker.authorize_with_human_approval(&intent, second, "session", 7, "wrong-call"),
        Decision::RequireHuman { .. }
    ));
}
