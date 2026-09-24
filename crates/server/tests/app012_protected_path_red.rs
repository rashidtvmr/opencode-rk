//! APP-012 RED: file reads stay inside the workspace and deny protected aliases.
//!
//! These tests use the public PermissionBroker plus file_ops path that opens
//! bytes. The live server journey already proves dispatch; this boundary keeps
//! fixtures disposable and makes denial leakage assertions direct.
#![forbid(unsafe_code)]

use std::{fs, path::Path};

use opencode_rk_security::{
    Decision, FileAction, OperationIntent, PermissionBroker, PermissionRule, PermissionSet,
    RuleEffect, SecurityPolicy,
};
use opencode_rk_tools::file_ops::{execute_authorized, FileOperation};
use tempfile::tempdir;

const DENIAL: &str = "file read denied";
const SAFE_BYTES: &[u8] = b"workspace-safe-bytes";
const OUTSIDE_BYTES: &[u8] = b"outside-canary-must-not-leak";
const ENV_BYTES: &[u8] = b"ENV_CANARY_MUST_NOT_LEAK";

fn broker(root: &Path) -> PermissionBroker {
    PermissionBroker::new(SecurityPolicy::lean_default(root))
}

fn read_intent(path: &Path) -> OperationIntent {
    OperationIntent::File {
        action: FileAction::Read,
        path: path.to_path_buf(),
    }
}

fn assert_denied(root: &Path, path: &Path, canary: &[u8]) {
    let broker = broker(root);
    let result = execute_authorized(FileOperation::read(path), &broker)
        .expect("protected-path read returns a bounded result");

    assert!(!result.success, "protected path unexpectedly read: {path:?}");
    assert_eq!(result.content, "", "denial must not return file content");
    assert_eq!(result.error.as_deref(), Some(DENIAL));
    assert!(!result.content.contains(std::str::from_utf8(canary).unwrap()));
    assert_eq!(broker.audit_len(), 1, "one broker denial per attempted read");
    let audit = broker.audit();
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].intent, read_intent(path));
    assert!(matches!(audit[0].decision, Decision::Deny { .. }));
}

#[test]
fn app012_workspace_read_succeeds() {
    let fixture = tempdir().expect("disposable fixture");
    let workspace = fixture.path().join("workspace");
    fs::create_dir_all(&workspace).expect("workspace directory");

    let safe = workspace.join("safe.txt");
    fs::write(&safe, SAFE_BYTES).expect("safe fixture");

    let allowed = broker(&workspace);
    let safe_result = execute_authorized(FileOperation::read(&safe), &allowed)
        .expect("workspace read result");
    assert!(safe_result.success);
    assert_eq!(safe_result.content.as_bytes(), SAFE_BYTES);
    assert!(safe_result.error.is_none());
}

#[test]
fn app012_workspace_env_read_is_redacted() {
    let fixture = tempdir().expect("disposable fixture");
    let workspace = fixture.path().join("workspace");
    fs::create_dir_all(&workspace).expect("workspace directory");
    let env_file = workspace.join(".env");
    fs::write(&env_file, ENV_BYTES).expect("env fixture");

    assert_denied(&workspace, &env_file, ENV_BYTES);
    assert_eq!(fs::read(&env_file).expect("env remains unchanged"), ENV_BYTES);
}

#[test]
fn app012_absolute_outside_read_is_redacted() {
    let fixture = tempdir().expect("disposable fixture");
    let workspace = fixture.path().join("workspace");
    let outside = fixture.path().join("outside");
    fs::create_dir_all(&workspace).expect("workspace directory");
    fs::create_dir_all(&outside).expect("outside directory");
    let outside_file = outside.join("ordinary.txt");
    fs::write(&outside_file, OUTSIDE_BYTES).expect("outside fixture");

    assert_denied(&workspace, &outside_file, OUTSIDE_BYTES);
    assert_eq!(
        fs::read(&outside_file).expect("outside fixture remains unchanged"),
        OUTSIDE_BYTES
    );
}

#[test]
fn app012_parent_traversal_read_is_redacted() {
    let fixture = tempdir().expect("disposable fixture");
    let workspace = fixture.path().join("workspace");
    let outside = fixture.path().join("outside");
    fs::create_dir_all(&workspace).expect("workspace directory");
    fs::create_dir_all(&outside).expect("outside directory");
    let outside_file = outside.join("ordinary.txt");
    fs::write(&outside_file, OUTSIDE_BYTES).expect("outside fixture");

    let traversal = workspace.join("..").join("outside").join("ordinary.txt");
    assert_denied(&workspace, &traversal, OUTSIDE_BYTES);
    assert_eq!(
        fs::read(&outside_file).expect("outside fixture remains unchanged"),
        OUTSIDE_BYTES
    );
}

#[test]
fn app012_wildcard_and_human_allow_cannot_bypass_env_denial() {
    let fixture = tempdir().expect("disposable fixture");
    let workspace = fixture.path().join("workspace");
    fs::create_dir_all(&workspace).expect("workspace directory");
    let env_file = workspace.join(".env");
    fs::write(&env_file, ENV_BYTES).expect("env fixture");

    let star = broker(&workspace).with_permissions(PermissionSet::star());
    assert!(matches!(
        star.authorize(&read_intent(&env_file)),
        Decision::Deny { .. }
    ));

    let human_allow = broker(&workspace).with_permissions(PermissionSet::new(vec![
        PermissionRule::new("**/.env", RuleEffect::Allow),
    ]));
    assert!(matches!(
        human_allow.authorize(&read_intent(&env_file)),
        Decision::Deny { .. }
    ));

    assert_denied(&workspace, &env_file, ENV_BYTES);
}

#[cfg(unix)]
#[test]
fn app012_symlinks_inside_workspace_never_follow_outside_or_broken_targets() {
    use std::os::unix::fs::symlink;

    let fixture = tempdir().expect("disposable fixture");
    let workspace = fixture.path().join("workspace");
    let outside = fixture.path().join("outside");
    fs::create_dir_all(&workspace).expect("workspace directory");
    fs::create_dir_all(&outside).expect("outside directory");

    let ordinary = outside.join("ordinary.txt");
    let protected = outside.join(".env");
    fs::write(&ordinary, OUTSIDE_BYTES).expect("outside ordinary fixture");
    fs::write(&protected, ENV_BYTES).expect("outside protected fixture");

    let ordinary_link = workspace.join("ordinary-link.txt");
    let protected_link = workspace.join("protected-link.txt");
    let broken_link = workspace.join("broken-link.txt");
    symlink(&ordinary, &ordinary_link).expect("ordinary symlink");
    symlink(&protected, &protected_link).expect("protected symlink");
    symlink(outside.join("missing.txt"), &broken_link).expect("broken symlink");

    assert_denied(&workspace, &ordinary_link, OUTSIDE_BYTES);
    assert_denied(&workspace, &protected_link, ENV_BYTES);
    assert_denied(&workspace, &broken_link, b"broken-link-canary");
    assert_eq!(fs::read(&ordinary).expect("ordinary target remains"), OUTSIDE_BYTES);
    assert_eq!(fs::read(&protected).expect("protected target remains"), ENV_BYTES);
}

#[cfg(unix)]
#[test]
fn app012_explicit_system_read_compatibility_remains_broker_visible() {
    let fixture = tempdir().expect("disposable fixture");
    let workspace = fixture.path().join("workspace");
    fs::create_dir_all(&workspace).expect("workspace directory");

    // The approved compatibility is a policy decision only. Do not probe a
    // host system file in this disposable-fixture suite.
    let mut policy = SecurityPolicy::lean_default(&workspace);
    policy.system_readable = true;
    let compatibility = PermissionBroker::new(policy);
    let decision = compatibility.authorize(&read_intent(Path::new("/etc/hosts")));
    assert_eq!(decision, Decision::Allow);
}
