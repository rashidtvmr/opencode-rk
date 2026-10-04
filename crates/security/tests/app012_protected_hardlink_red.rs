//! PATH-HARDLINK-RED: lexical path authorization must not let a workspace
//! hardlink alias bypass mandatory protected-file denial.
#![forbid(unsafe_code)]

use opencode_rk_security::{
    Decision, FileAction, OperationIntent, PermissionBroker, PermissionSet, SecurityPolicy,
};
use std::{fs, path::PathBuf};

struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("rk-app012-hardlink-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let workspace = root.join("workspace");
        fs::create_dir_all(&workspace).unwrap();
        Self { root, workspace }
    }

    fn broker(&self) -> PermissionBroker {
        PermissionBroker::new(SecurityPolicy::lean_default(&self.workspace))
            .with_permissions(PermissionSet::star())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn read(path: PathBuf) -> OperationIntent {
    OperationIntent::File {
        action: FileAction::Read,
        path,
    }
}

#[test]
fn single_link_workspace_file_remains_readable() {
    let fixture = Fixture::new("safe");
    let safe = fixture.workspace.join("safe.txt");
    fs::write(&safe, b"ordinary fixture").unwrap();

    assert_eq!(fixture.broker().authorize(&read(safe)), Decision::Allow);
}

#[test]
fn hardlink_alias_to_protected_file_is_mandatorily_denied() {
    let fixture = Fixture::new("protected-alias");
    let protected = fixture.root.join(".env");
    let alias = fixture.workspace.join("apparently-safe.txt");
    fs::write(&protected, b"disposable-canary").unwrap();
    fs::hard_link(&protected, &alias).unwrap();

    let broker = fixture.broker();
    let decision = broker.authorize(&read(alias));
    assert!(
        matches!(decision, Decision::Deny { .. }),
        "wildcard permission must not authorize a multi-link alias"
    );
    assert_eq!(broker.audit_len(), 1, "one bounded denial must be audited");
}
