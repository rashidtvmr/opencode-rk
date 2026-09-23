use opencode_rk_server::daemon::DaemonPaths;
use std::fs;
use tempfile::tempdir;

/// Darwin's `sockaddr_un.sun_path` is 104 bytes, including its NUL terminator.
/// `< 104` therefore leaves a conservative pathname bound usable by `SUN_LEN`.
const DARWIN_SUN_LEN_BYTES: usize = 104;

#[test]
fn daemon_socket_path_stays_below_darwin_sun_len_for_long_data_dirs() {
    let root = tempdir().expect("disposable temporary root");
    let long_suffix = "test-home-with-a-deliberately-long-name-0123456789";
    let data_a = root
        .path()
        .join("macos-tmpdir-with-a-long-prefix")
        .join(long_suffix)
        .join("Library")
        .join("Application Support")
        .join("OpenCode RK")
        .join("profile-a");
    let data_b = data_a.with_file_name("profile-b");
    fs::create_dir_all(&data_a).expect("create disposable long data directory");
    fs::create_dir_all(&data_b).expect("create isolated second data directory");

    let canonical_a = fs::canonicalize(&data_a).expect("canonicalize first data directory");
    let canonical_b = fs::canonicalize(&data_b).expect("canonicalize second data directory");
    let paths_a = DaemonPaths::for_data_dir(&canonical_a);
    let paths_a_again = DaemonPaths::for_data_dir(&canonical_a);
    let paths_b = DaemonPaths::for_data_dir(&canonical_b);

    let socket_a = paths_a.socket.to_string_lossy();
    assert!(
        socket_a.len() < DARWIN_SUN_LEN_BYTES,
        "derived Unix socket path must be shorter than Darwin SUN_LEN: {} bytes >= {DARWIN_SUN_LEN_BYTES}: {socket_a}",
        socket_a.len()
    );
    assert_eq!(paths_a.socket, paths_a_again.socket);
    assert_ne!(paths_a.socket, paths_b.socket);

    assert!(paths_a.pid.starts_with(&canonical_a));
    assert!(paths_a.descriptor.starts_with(&canonical_a));
    assert!(paths_b.pid.starts_with(&canonical_b));
    assert!(paths_b.descriptor.starts_with(&canonical_b));
}
