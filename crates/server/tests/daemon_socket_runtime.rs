#![cfg(unix)]

use opencode_rk_server::daemon::{DaemonError, DaemonPaths, SingletonDaemon};
use std::{
    fs,
    os::unix::fs::DirBuilderExt,
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, PermissionsExt},
    },
    path::Path,
    time::Duration,
};
use tempfile::TempDir;
use tokio::{net::UnixStream, time::timeout};

fn private_fixture() -> (TempDir, u32) {
    let dir = tempfile::Builder::new()
        .prefix("g1-")
        .tempdir()
        .expect("disposable daemon data directory");
    let root = fs::symlink_metadata(dir.path()).expect("inspect fixture root");
    assert!(root.is_dir());
    assert!(!root.file_type().is_symlink());
    assert_eq!(
        root.permissions().mode() & 0o022,
        0,
        "fixture root is private"
    );
    (dir, root.uid())
}

fn inspect_private_parent(path: &Path, fixture_uid: u32) {
    let metadata = fs::symlink_metadata(path).expect("inspect endpoint parent");
    assert!(metadata.is_dir());
    assert!(!metadata.file_type().is_symlink());
    assert_eq!(
        metadata.permissions().mode() & 0o022,
        0,
        "endpoint parent is private"
    );
    assert_eq!(
        metadata.uid(),
        fixture_uid,
        "endpoint parent owner matches fixture"
    );
}

fn regular_bytes(path: &Path, expected: &[u8]) {
    let metadata = fs::symlink_metadata(path).expect("sentinel remains");
    assert!(metadata.is_file());
    assert!(!metadata.file_type().is_symlink());
    assert_eq!(fs::read(path).unwrap(), expected);
}

#[tokio::test]
async fn long_data_directory_uses_short_real_socket_and_rebinds() {
    let (fixture, fixture_uid) = private_fixture();
    let mut data_dir = fixture.path().to_path_buf();
    for component in [
        "alpha-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "beta-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "gamma-cccccccccccccccccccccccccccccccc",
        "delta-dddddddddddddddddddddddddddddddd",
    ] {
        data_dir.push(component);
    }
    while data_dir.as_os_str().as_bytes().len() <= 200 {
        data_dir.push("epsilon-eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
    }
    fs::create_dir_all(&data_dir).expect("create nested disposable data directory");
    assert!(data_dir.as_os_str().as_bytes().len() > 200);
    let paths = DaemonPaths::for_data_dir(&data_dir);
    assert!(paths.socket.as_os_str().as_bytes().len() < 104);

    let daemon = SingletonDaemon::bind(&paths.socket, &paths.pid).expect("bind real socket");
    let runtime_parent = paths.socket.parent().unwrap();
    let runtime_meta = fs::symlink_metadata(runtime_parent).unwrap();
    assert!(runtime_meta.is_dir());
    assert!(!runtime_meta.file_type().is_symlink());
    assert_eq!(runtime_meta.permissions().mode() & 0o077, 0);
    assert_eq!(runtime_meta.uid(), fixture_uid);
    timeout(Duration::from_secs(2), UnixStream::connect(&paths.socket))
        .await
        .expect("socket connect deadline")
        .expect("connect real socket");
    assert!(matches!(
        SingletonDaemon::bind(&paths.socket, &paths.pid),
        Err(DaemonError::AlreadyRunning(_))
    ));
    drop(daemon);
    assert!(!paths.socket.exists());
    let rebound = SingletonDaemon::bind(&paths.socket, &paths.pid).expect("rebind real socket");
    timeout(Duration::from_secs(2), UnixStream::connect(&paths.socket))
        .await
        .expect("rebound socket connect deadline")
        .expect("connect rebound socket");
    drop(rebound);
}

#[tokio::test]
async fn regular_sentinel_endpoint_is_rejected_without_mutation() {
    let (fixture, fixture_uid) = private_fixture();
    let paths = DaemonPaths::for_data_dir(fixture.path());
    assert!(paths.socket.starts_with(fixture.path()));
    assert!(paths.socket.as_os_str().as_bytes().len() < 104);
    let parent = paths.socket.parent().unwrap();
    assert!(parent.starts_with(fixture.path()));
    fs::DirBuilder::new().mode(0o700).create(parent).unwrap();
    inspect_private_parent(parent, fixture_uid);
    let sentinel = b"do-not-delete-runtime-sentinel";
    fs::write(&paths.socket, sentinel).unwrap();
    let result = SingletonDaemon::bind(&paths.socket, &paths.pid);
    assert!(matches!(result, Err(DaemonError::Io(_))));
    regular_bytes(&paths.socket, sentinel);
    assert!(!paths.pid.exists());
}

#[tokio::test]
async fn symlink_endpoint_is_rejected_without_target_or_identity_mutation() {
    let (fixture, fixture_uid) = private_fixture();
    let paths = DaemonPaths::for_data_dir(fixture.path());
    assert!(paths.socket.starts_with(fixture.path()));
    assert!(paths.socket.as_os_str().as_bytes().len() < 104);
    let parent = paths.socket.parent().unwrap();
    assert!(parent.starts_with(fixture.path()));
    fs::DirBuilder::new().mode(0o700).create(parent).unwrap();
    inspect_private_parent(parent, fixture_uid);
    let target = fixture.path().join("target");
    let bytes = b"preserve-this-target";
    fs::write(&target, bytes).unwrap();
    std::os::unix::fs::symlink(&target, &paths.socket).unwrap();
    let result = SingletonDaemon::bind(&paths.socket, &paths.pid);
    assert!(matches!(result, Err(DaemonError::Io(_))));
    let endpoint = fs::symlink_metadata(&paths.socket).unwrap();
    assert!(endpoint.file_type().is_symlink());
    assert_eq!(fs::read_link(&paths.socket).unwrap(), target);
    assert_eq!(fs::read(&target).unwrap(), bytes);
    assert!(!paths.pid.exists());
}
