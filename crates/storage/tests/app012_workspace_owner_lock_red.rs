//! OWN-LOCK-RED: the production storage facade must hold exclusive writable
//! workspace ownership for its lifetime and release it on orderly close.
#![forbid(unsafe_code)]

use opencode_rk_storage::facade::StorageFacade;

#[test]
fn second_writable_facade_is_denied_while_owner_is_live() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let owner = StorageFacade::open(&path).unwrap();

    let contender = StorageFacade::open(&path);
    assert!(
        contender.is_err(),
        "one live writable facade must exclude a second owner"
    );

    owner.close().unwrap();
}

#[test]
fn orderly_close_releases_workspace_for_next_owner() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.db");

    StorageFacade::open(&path).unwrap().close().unwrap();
    let successor = StorageFacade::open(&path);
    assert!(
        successor.is_ok(),
        "the ownership lifetime ends when the facade closes"
    );
    successor.unwrap().close().unwrap();
}
