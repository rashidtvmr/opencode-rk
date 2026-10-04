#![forbid(unsafe_code)]

use opencode_rk_security::{
    Decision, FileAction, OperationIntent, PermissionBroker, PermissionSet, SecurityPolicy,
};
use std::fs;

fn read(path: std::path::PathBuf) -> OperationIntent {
    OperationIntent::File {
        action: FileAction::Read,
        path,
    }
}

#[cfg(unix)]
#[test]
fn protected_hardlink_alias_through_workspace_symlink_is_mandatorily_denied() {
    use std::os::unix::fs::{symlink, MetadataExt};

    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().join("workspace");
    fs::create_dir(&workspace).unwrap();
    let protected = fixture.path().join("disposable-protected.env");
    let hardlink = workspace.join("apparently-benign.txt");
    let symlink_alias = workspace.join("apparently-benign-link.txt");
    fs::write(&protected, b"disposable canary only").unwrap();
    fs::hard_link(&protected, &hardlink).unwrap();
    symlink(&hardlink, &symlink_alias).unwrap();

    let link_metadata = fs::symlink_metadata(&symlink_alias).unwrap();
    assert!(link_metadata.file_type().is_symlink());
    assert_eq!(fs::metadata(&symlink_alias).unwrap().nlink(), 2);

    let broker = PermissionBroker::new(SecurityPolicy::lean_default(&workspace))
        .with_permissions(PermissionSet::star());
    let decision = broker.authorize(&read(symlink_alias));
    assert!(
        matches!(decision, Decision::Deny { .. }),
        "wildcard must not authorize protected hardlink through workspace symlink; got {decision:?}, audit_entries={}",
        broker.audit_len()
    );
    assert_eq!(broker.audit_len(), 1);
}

#[cfg(unix)]
#[test]
fn symlink_to_single_link_workspace_file_remains_readable() {
    use std::os::unix::fs::symlink;

    let fixture = tempfile::tempdir().unwrap();
    let workspace = fixture.path().join("workspace");
    fs::create_dir(&workspace).unwrap();
    let ordinary = workspace.join("ordinary.txt");
    let alias = workspace.join("ordinary-link.txt");
    fs::write(&ordinary, b"ordinary disposable fixture").unwrap();
    symlink(&ordinary, &alias).unwrap();

    let broker = PermissionBroker::new(SecurityPolicy::lean_default(&workspace))
        .with_permissions(PermissionSet::star());
    assert_eq!(broker.authorize(&read(alias)), Decision::Allow);
}
