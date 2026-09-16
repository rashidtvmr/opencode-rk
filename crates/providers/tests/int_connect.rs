//! INT-002 lane tests: caller-driven key/OAuth connection projection.
//!
//! Module under test is included by path so this lane never edits the
//! shared `crates/providers/src/lib.rs` (integrator-owned). Distinct lane
//! file from `integration_connection.rs` (different slice, oracle-count fix
//! pending elsewhere): this lane borrows `code` as `&str` and retains nothing.

#[path = "../src/int_connect.rs"]
mod int_connect;

use int_connect::{
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
fn int_connect_t01_key_and_oauth_with_code_connect_active_with_monotonic_ids() {
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
fn int_connect_t02_oauth_without_code_is_typed_and_records_nothing() {
    let known = oracle();
    let mut table = ConnectionTable::new();
    table
        .connect(&key_input(), &known)
        .expect("prior key connect should succeed");
    let before = format!("{:?}", table.list());

    let err = table
        .connect(&oauth_input(None), &known)
        .expect_err("oauth without code must be typed");
    assert_eq!(err, ConnectError::CodeRequired);
    assert_eq!(table.list().len(), 1);
    assert_eq!(format!("{:?}", table.list()), before);
}

#[test]
fn int_connect_t03_unknown_not_found_invalid_input_overflow_leave_table_unchanged() {
    assert_eq!(MAX_CONNECTIONS, 64);
    assert_eq!(MAX_CODE_LEN, 512);

    let known = oracle();
    let mut table = ConnectionTable::new();

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
    assert!(table.list().is_empty());

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
    assert!(table.list().is_empty());

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
fn int_connect_t04_disconnect_list_get() {
    let known = oracle();
    let mut table = ConnectionTable::new();
    table
        .connect(&key_input(), &known)
        .expect("key connect should succeed");
    table
        .connect(&oauth_input(Some("code-1")), &known)
        .expect("oauth connect with code should succeed");

    assert!(table.disconnect(ConnId(1)));
    assert_eq!(table.get(ConnId(1)), None);
    let ids: Vec<u64> = table.list().iter().map(|info| info.id.0).collect();
    assert_eq!(ids, vec![2]);

    assert!(!table.disconnect(ConnId(1)));
    let after: Vec<u64> = table.list().iter().map(|info| info.id.0).collect();
    assert_eq!(after, vec![2]);

    assert!(!table.disconnect(ConnId(999)));
    let still: Vec<u64> = table.list().iter().map(|info| info.id.0).collect();
    assert_eq!(still, vec![2]);
}

#[test]
fn int_connect_t05_no_retention_debug_secret_free_and_pure() {
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
    let mut attempts = 0usize;
    let mut log_capture = String::new();

    let ok_key = table.connect(&key_input(), &known);
    attempts += 1;
    log_capture.push_str(&format!("{:?} ", ok_key.as_ref().err()));
    assert!(ok_key.is_ok());

    let secret_code = "zxq-code-9-SECRET";
    let info = table.connect(&oauth_input(Some(secret_code)), &known);
    attempts += 1;
    log_capture.push_str(&format!("{:?} ", info.as_ref().err()));
    let info = info.expect("oauth connect with code should succeed");

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

    // Failure matrix; each error leaves typed logs only.
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
    log_capture.push_str(&format!("{:?}", table));

    // Oracle consulted exactly once per connect: no hidden retries/callbacks.
    assert_eq!(calls.get(), attempts);

    // Zero code bytes retained or logged: table, returned info, input debug.
    assert!(!format!("{:?}", table).contains(secret_code));
    let returned = table.get(ConnId(2)).expect("id 2 should exist");
    assert!(!format!("{:?}", returned).contains(secret_code));
    assert!(!format!("{:?}", oauth_input(Some(secret_code))).contains(secret_code));
    assert!(!log_capture.contains(secret_code));
}
