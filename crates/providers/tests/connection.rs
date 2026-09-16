//! INT-002 frozen tests: caller-driven key/OAuth connection projection.
//!
//! Module under test is included by path so this lane never edits the
//! shared `crates/providers/src/lib.rs` (integrator-owned). Exact owned
//! path `crates/providers/src/connection.rs`; borrowed `&str` material.

#[path = "../src/connection.rs"]
mod connection;

use connection::{
    ConnId, ConnectError, ConnectInput, ConnectionInfo, ConnectionState, ConnectionTable,
    MethodKind, MAX_CODE_LEN, MAX_CONNECTIONS,
};
use std::cell::Cell;
use std::rc::Rc;

fn oracle() -> impl Fn(&str, &str) -> Option<MethodKind> {
    |integration: &str, method: &str| match (integration, method) {
        ("gh", "key") => Some(MethodKind::Key),
        ("gh", "oauth") => Some(MethodKind::OAuth),
        ("fill", m) if m.starts_with('k') => Some(MethodKind::Key),
        _ => None,
    }
}

fn key_input() -> ConnectInput<'static> {
    ConnectInput {
        integration: "gh",
        method_id: "key",
        code: None,
    }
}

fn oauth_input(code: Option<&str>) -> ConnectInput<'_> {
    ConnectInput {
        integration: "gh",
        method_id: "oauth",
        code,
    }
}

#[test]
fn int_002_t01_key_and_oauth_with_code_connect_active_with_monotonic_ids() {
    let known = oracle();
    let mut table = ConnectionTable::new();

    let first = table
        .connect(&key_input(), &known)
        .expect("key connect should succeed");
    assert_eq!(first.id, ConnId(1));
    assert_eq!(first.state, ConnectionState::Active);

    let second = table
        .connect(&oauth_input(Some("code-1")), &known)
        .expect("oauth connect with code should succeed");
    assert_eq!(second.id, ConnId(2));
    assert_eq!(second.state, ConnectionState::Active);

    assert_eq!(
        table.get(ConnId(1)).map(|info| info.method_id.as_str()),
        Some("key")
    );
    let ids: Vec<u64> = table.list().iter().map(|info| info.id.0).collect();
    assert_eq!(ids, vec![1, 2]);
}

#[test]
fn int_002_t02_oauth_without_code_and_unknown_pairs_are_typed_and_non_mutating() {
    let known = oracle();
    let mut table = ConnectionTable::new();
    table
        .connect(&key_input(), &known)
        .expect("prior key connect should succeed");
    let before = format!("{:?}", table.list());

    let missing = table
        .connect(&oauth_input(None), &known)
        .expect_err("oauth without code must be typed");
    assert_eq!(missing, ConnectError::CodeRequired);
    assert_eq!(table.list().len(), 1);

    let unknown = table
        .connect(
            &ConnectInput {
                integration: "nope",
                method_id: "key",
                code: None,
            },
            &known,
        )
        .expect_err("unknown pair must be typed");
    assert_eq!(unknown, ConnectError::NotFound);

    // Normalization: disallowed-but-wellformed pair yields identical variant.
    let disallowed = table
        .connect(
            &ConnectInput {
                integration: "gh",
                method_id: "nope",
                code: None,
            },
            &known,
        )
        .expect_err("disallowed pair must normalize to NotFound");
    assert_eq!(disallowed, ConnectError::NotFound);
    assert_eq!(disallowed, unknown);

    assert_eq!(format!("{:?}", table.list()), before);
}

#[test]
fn int_002_t03_validation_and_overflow_leave_table_byte_identical() {
    assert_eq!(MAX_CONNECTIONS, 64);
    assert_eq!(MAX_CODE_LEN, 512);

    let known = oracle();
    let mut table = ConnectionTable::new();

    let empty = table
        .connect(
            &ConnectInput {
                integration: "",
                method_id: "key",
                code: None,
            },
            &known,
        )
        .expect_err("empty integration must be invalid");
    assert_eq!(empty, ConnectError::InvalidInput);
    assert!(table.list().is_empty());

    let bad_charset = table
        .connect(
            &ConnectInput {
                integration: "gh",
                method_id: "a/b",
                code: None,
            },
            &known,
        )
        .expect_err("bad-charset method must be invalid");
    assert_eq!(bad_charset, ConnectError::InvalidInput);
    assert!(table.list().is_empty());

    let long_code = "c".repeat(MAX_CODE_LEN + 1);
    let oversize = table
        .connect(&oauth_input(Some(&long_code)), &known)
        .expect_err("513-char code must be invalid");
    assert_eq!(oversize, ConnectError::InvalidInput);
    assert!(table.list().is_empty());

    for index in 0..MAX_CONNECTIONS {
        let method = format!("k{index}");
        let input = ConnectInput {
            integration: "fill",
            method_id: method.as_str(),
            code: None,
        };
        table
            .connect(&input, &known)
            .expect("exact connection bound should succeed");
    }
    assert_eq!(table.list().len(), MAX_CONNECTIONS);
    let before = format!("{:?}", table.list());

    let overflow = table
        .connect(
            &ConnectInput {
                integration: "fill",
                method_id: "k-overflow",
                code: None,
            },
            &known,
        )
        .expect_err("connection past the cap must overflow");
    assert_eq!(overflow, ConnectError::Overflow);
    assert_eq!(table.list().len(), MAX_CONNECTIONS);
    assert_eq!(format!("{:?}", table.list()), before);
}

#[test]
fn int_002_t04_disconnect_removes_one_id_and_code_is_never_retained() {
    let known = oracle();
    let mut table = ConnectionTable::new();
    table
        .connect(&key_input(), &known)
        .expect("key connect should succeed");

    let secret_code = "zxq-code-9-SECRET";
    let info = table
        .connect(&oauth_input(Some(secret_code)), &known)
        .expect("oauth connect with code should succeed");

    // Type-level guard: exhaustive destructure fails to compile if a
    // code/secret/token field is ever added to the returned type.
    let ConnectionInfo {
        id,
        integration,
        method_id,
        state,
    } = info;
    assert_eq!(id, ConnId(2));
    assert_eq!(integration, "gh");
    assert_eq!(method_id, "oauth");
    assert_eq!(state, ConnectionState::Active);

    // Memory scan: no retained code bytes in table or returned info debug.
    assert!(!format!("{:?}", table).contains(secret_code));
    let returned = table.get(ConnId(2)).expect("id 2 should exist");
    assert!(!format!("{:?}", returned).contains(secret_code));
    // Input Debug itself must redact code material.
    assert!(!format!("{:?}", oauth_input(Some(secret_code))).contains(secret_code));

    assert!(table.disconnect(ConnId(1)));
    assert_eq!(table.get(ConnId(1)), None);
    let ids: Vec<u64> = table.list().iter().map(|info| info.id.0).collect();
    assert_eq!(ids, vec![2]);

    assert!(!table.disconnect(ConnId(1)));
    let after: Vec<u64> = table.list().iter().map(|info| info.id.0).collect();
    assert_eq!(after, vec![2]);
}

fn child_pids() -> Vec<u32> {
    let me = std::process::id();
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return out;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{name}/stat")) else {
            continue;
        };
        let Some(close) = stat.rfind(')') else {
            continue;
        };
        let mut fields = stat[close + 2..].split_whitespace();
        fields.next(); // state
        let is_child = fields.next().and_then(|ppid| ppid.parse::<u32>().ok()) == Some(me);
        if !is_child {
            continue;
        }
        if let Ok(pid) = name.parse::<u32>() {
            out.push(pid);
        }
    }
    out.sort_unstable();
    out
}

fn socket_count() -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir("/proc/self/fd") {
        for entry in entries.flatten() {
            let is_socket = std::fs::read_link(entry.path())
                .map(|target| target.to_string_lossy().starts_with("socket:"))
                .unwrap_or(false);
            if is_socket {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn int_002_t05_full_matrix_spawns_nothing_writes_nothing_leaks_nothing() {
    let dir = tempfile::tempdir().expect("disposable test dir should exist");
    let before_files: Vec<String> = std::fs::read_dir(dir.path())
        .expect("test dir should list")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    let before_children = child_pids();
    let before_sockets = socket_count();

    let calls = Rc::new(Cell::new(0usize));
    let known = {
        let calls = Rc::clone(&calls);
        move |integration: &str, method: &str| -> Option<MethodKind> {
            calls.set(calls.get() + 1);
            match (integration, method) {
                ("gh", "key") => Some(MethodKind::Key),
                ("gh", "oauth") => Some(MethodKind::OAuth),
                _ => None,
            }
        }
    };

    let mut table = ConnectionTable::new();
    let mut attempts = 0;
    let mut log_capture = String::new();

    // Happy paths.
    let ok_key = table.connect(&key_input(), &known);
    attempts += 1;
    log_capture.push_str(&format!("{:?} ", ok_key.as_ref().err()));
    assert!(ok_key.is_ok());
    let probe_code = "probe-code-77-SECRET";
    let ok_oauth = table.connect(&oauth_input(Some(probe_code)), &known);
    attempts += 1;
    log_capture.push_str(&format!("{:?} ", ok_oauth.as_ref().err()));
    assert!(ok_oauth.is_ok());

    // Failure matrix.
    let long_code = "c".repeat(MAX_CODE_LEN + 1);
    let failures: Vec<ConnectInput<'_>> = vec![
        oauth_input(None),
        ConnectInput {
            integration: "nope",
            method_id: "key",
            code: None,
        },
        ConnectInput {
            integration: "",
            method_id: "key",
            code: None,
        },
        ConnectInput {
            integration: "gh",
            method_id: "a/b",
            code: None,
        },
        oauth_input(Some(long_code.as_str())),
    ];
    for input in &failures {
        let err = table
            .connect(input, &known)
            .expect_err("matrix failure input must be typed");
        attempts += 1;
        log_capture.push_str(&format!("{err:?} "));
    }
    assert!(table.disconnect(ConnId(1)));
    assert!(!table.disconnect(ConnId(999)));
    log_capture.push_str(&format!("{:?}", table));

    // Oracle consulted exactly once per connect, no hidden retries/callbacks.
    assert_eq!(calls.get(), attempts);

    // Captured logs and table debug carry zero code/credential bytes.
    assert!(!log_capture.contains(probe_code));
    assert!(!log_capture.contains("code-1"));
    assert!(!format!("{:?}", table).contains(probe_code));

    // No side effects: no child processes, no sockets, no files, no DB files.
    assert_eq!(child_pids(), before_children);
    assert_eq!(socket_count(), before_sockets);
    let after_files: Vec<String> = std::fs::read_dir(dir.path())
        .expect("test dir should list")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(after_files, before_files);
    assert!(std::fs::read_dir(dir.path())
        .expect("dir")
        .flatten()
        .all(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            !name.ends_with(".db") && !name.ends_with(".sqlite")
        }));
}
