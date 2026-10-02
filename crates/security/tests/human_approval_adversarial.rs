//! Human approval must not turn an unrelated or mandatory-denied operation into Allow.
//!
//! These tests assert that the generation-only `authorize_human_approved` API
//! cannot confer scoped human authority: it cannot lift a Deny, and a stale
//! generation cannot retroactively Allow an unrelated operation. The tests are
//! intentionally RED against the current product where
//! `authorize_human_approved` skips permission rules when the baseline is
//! `RequireHuman`, allowing a matching Deny rule to be bypassed.
#![forbid(unsafe_code)]

use opencode_rk_security::{
    Decision, FileAction, OperationIntent, PermissionBroker, PermissionRule, PermissionSet,
    RuleEffect, SecurityPolicy,
};
use std::path::PathBuf;
use tempfile::tempdir;

fn write(path: PathBuf) -> OperationIntent {
    OperationIntent::File {
        action: FileAction::Write,
        path,
    }
}

#[test]
fn approval_generation_is_bound_to_exact_operation_and_policy_generation() {
    let root = tempdir().unwrap();
    let other = tempdir().unwrap();
    let broker = PermissionBroker::new(SecurityPolicy::lean_default(root.path()));
    let generation = broker.generation();
    let approved = write(root.path().join("approved.txt"));
    let outside = write(other.path().join("outside.txt"));

    assert_eq!(broker.authorize(&approved), Decision::Allow);

    // Generation-only API: authorize_human_approved with a matching generation
    // cannot confer authority for an operation that was never scoped to this
    // generation. The `outside` write is unrelated to `approved`; approval for
    // `approved` at `generation` does not authorize `outside`.
    assert!(matches!(
        broker.authorize(&outside),
        Decision::RequireHuman { .. }
    ));
    assert!(matches!(
        broker.authorize_human_approved(&outside, generation),
        Decision::RequireHuman { .. }
    ));

    // Exact policy-generation stale proof: bumping the policy (which bumps
    // generation) invalidates any prior generation-only approval.
    let changed = broker.with_policy(SecurityPolicy::lean_default(other.path()));
    assert_ne!(changed.generation(), generation);
    assert!(matches!(
        changed.authorize_human_approved(&outside, generation),
        Decision::RequireHuman { .. }
    ));
}

#[test]
fn human_approval_cannot_bypass_explicit_deny_or_mandatory_protected_paths() {
    let root = tempdir().unwrap();
    let other = tempdir().unwrap();
    let outside_path = other.path().join("explicit.txt");

    // Explicit deny rule-match prerequisite: the "*" glob matches every
    // OperationIntent under library semantics (PermissionRule::matches).
    let deny_rule = PermissionRule::new("*", RuleEffect::Deny);
    assert!(
        deny_rule.matches(&write(outside_path.clone())),
        "deny rule \"*\" must match out-of-root write intent"
    );
    assert!(matches!(
        PermissionSet::new(vec![deny_rule.clone()]).evaluate(&write(outside_path.clone())),
        Some(rule) if rule.effect == RuleEffect::Deny
    ));

    // Establish the baseline on a separate broker without permission rules:
    // an out-of-root write requires human approval.
    let baseline = PermissionBroker::new(SecurityPolicy::lean_default(root.path()));
    let explicit = write(outside_path);
    assert!(matches!(
        baseline.authorize(&explicit),
        Decision::RequireHuman { .. }
    ));

    // The matched broker carries the explicit Deny as well as mandatory
    // protection (secret paths, system paths). The explicit rule must win over
    // the same baseline RequireHuman result.
    let broker = PermissionBroker::new(SecurityPolicy::lean_default(root.path()))
        .with_permissions(PermissionSet::new(vec![deny_rule]));
    let generation = broker.generation();

    // The matched rule returns Deny; generation-only approval cannot bypass it.
    assert!(matches!(
        broker.authorize(&explicit),
        Decision::Deny { .. }
    ));

    // The current product bug: authorize_human_approved skips the permission
    // rule check when baseline is RequireHuman, returning Allow instead of Deny.
    // This assertion must hold: human approval cannot bypass an explicit Deny.
    assert!(matches!(
        broker.authorize_human_approved(&explicit, generation),
        Decision::Deny { .. }
    ));

    // Mandatory protection: secret paths are denied regardless of generation.
    let secret = write(PathBuf::from("/etc/shadow"));
    assert!(matches!(
        broker.authorize_human_approved(&secret, generation),
        Decision::Deny { .. }
    ));
}
