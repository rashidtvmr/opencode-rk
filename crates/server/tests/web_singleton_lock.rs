use opencode_rk_server::daemon::{
    publish_backend_descriptor, read_backend_descriptor, DaemonPaths, PidLock,
};
use std::{
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, Barrier},
    thread,
    time::Duration,
};
use tempfile::tempdir;

#[test]
fn web_006_t02_stale_pid_and_descriptor_are_recoverable() {
    let dir = tempdir().expect("temporary daemon data directory");
    let paths = DaemonPaths::for_data_dir(dir.path());
    fs::create_dir_all(paths.pid.parent().expect("runtime directory")).unwrap();
    fs::write(&paths.pid, u32::MAX.to_string()).expect("write stale pid payload");

    let lock = PidLock::acquire(&paths.pid).expect("stale unlocked pid file must be reusable");
    assert!(PidLock::is_held(&paths.pid));
    drop(lock);
    assert!(!PidLock::is_held(&paths.pid));

    let published = publish_backend_descriptor(
        dir.path(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 41001),
    )
    .expect("publish current descriptor");
    assert_eq!(
        read_backend_descriptor(dir.path()).unwrap(),
        Some(published.clone())
    );

    let mut stale = published;
    stale.pid = u32::MAX;
    fs::write(&paths.descriptor, serde_json::to_vec(&stale).unwrap()).unwrap();
    assert_eq!(read_backend_descriptor(dir.path()).unwrap(), None);
}

#[test]
fn web_006_t04_concurrent_acquisition_has_exactly_one_owner() {
    let dir = tempdir().expect("temporary daemon data directory");
    let lock_path = Arc::new(DaemonPaths::for_data_dir(dir.path()).pid);
    let barrier = Arc::new(Barrier::new(3));

    let mut handles = Vec::new();
    for _ in 0..2 {
        let lock_path = Arc::clone(&lock_path);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            match PidLock::acquire(lock_path.as_path()) {
                Ok(lock) => {
                    thread::sleep(Duration::from_millis(100));
                    drop(lock);
                    true
                }
                Err(_) => false,
            }
        }));
    }

    barrier.wait();
    let winners = handles
        .into_iter()
        .map(|handle| handle.join().expect("acquirer thread"))
        .filter(|owned| *owned)
        .count();
    assert_eq!(
        winners, 1,
        "exactly one concurrent starter may own the daemon"
    );
}
