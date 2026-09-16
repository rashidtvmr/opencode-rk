//! INT-003 frozen tests T01..T05: provider-specific method dispatch boundary.
//!
//! The module under test is included by path so this lane never edits the
//! shared `crates/providers/src/lib.rs` (integrator-owned).

#[path = "../src/int_methods.rs"]
mod int_methods;

use int_methods::{MethodError, MethodInput, MethodTable, MAX_METHODS_PER_PROVIDER, MAX_PROVIDERS};
use std::cell::Cell;
use std::rc::Rc;

fn input(handle: &str) -> MethodInput<'_> {
    MethodInput {
        handle,
        bytes: b"opaque-handle-bytes",
    }
}

fn reg(table: &mut MethodTable, provider: &str, method: &str, handler: u64) {
    table
        .register(provider, method, handler)
        .expect("register should succeed");
}

#[test]
fn int_003_t01_register_dispatch_happy_path() {
    let mut table = MethodTable::new();
    reg(&mut table, "prov-a", "key", 11);
    reg(&mut table, "prov-a", "oauth", 12);
    reg(&mut table, "prov-b", "key", 21);
    reg(&mut table, "prov-b", "sync-now", 22);

    assert_eq!(table.providers().len(), 2);
    assert_eq!(table.list_methods("prov-a").len(), 2);
    assert_eq!(table.list_methods("prov-b").len(), 2);

    let receipt = table
        .dispatch("prov-a", "key", &input("h1"))
        .expect("dispatch");
    assert_eq!(receipt.handler_id, 11);
    assert!(receipt.accepted);
    let receipt = table
        .dispatch("prov-b", "sync-now", &input("h2"))
        .expect("dispatch");
    assert_eq!(receipt.handler_id, 22);
    assert!(receipt.accepted);
}

#[test]
fn int_003_t02_determinism_ordering_duplicate_replaces() {
    let mut t1 = MethodTable::new();
    reg(&mut t1, "prov-a", "key", 1);
    reg(&mut t1, "prov-a", "oauth", 2);
    reg(&mut t1, "prov-b", "key", 3);

    let mut t2 = MethodTable::new();
    reg(&mut t2, "prov-b", "key", 3);
    reg(&mut t2, "prov-a", "oauth", 2);
    reg(&mut t2, "prov-a", "key", 1);

    assert_eq!(t1.providers(), t2.providers());
    assert_eq!(t1.list_methods("prov-a"), t2.list_methods("prov-a"));
    assert_eq!(t1.list_methods("prov-b"), t2.list_methods("prov-b"));

    let before = t1.len();
    reg(&mut t1, "prov-a", "key", 99);
    assert_eq!(t1.len(), before);
    let receipt = t1.dispatch("prov-a", "key", &input("h")).expect("dispatch");
    assert_eq!(receipt.handler_id, 99);
    assert!(receipt.accepted);
}

#[test]
fn int_003_t03_caps_hold_and_bulk_dispatch_stable() {
    let mut table = MethodTable::new();
    for i in 0..MAX_PROVIDERS {
        reg(&mut table, &format!("prov-{i:02}"), "key", i as u64);
    }
    assert_eq!(table.providers().len(), MAX_PROVIDERS);
    assert_eq!(table.len(), MAX_PROVIDERS);
    match table.register("prov-overflow", "key", 999) {
        Err(MethodError::TableFull) => {}
        other => panic!("expected TableFull, got {other:?}"),
    }
    assert_eq!(table.len(), MAX_PROVIDERS);

    let mut per_provider = MethodTable::new();
    for i in 0..MAX_METHODS_PER_PROVIDER {
        reg(&mut per_provider, "solo", &format!("m-{i:02}"), i as u64);
    }
    assert_eq!(
        per_provider.list_methods("solo").len(),
        MAX_METHODS_PER_PROVIDER
    );
    match per_provider.register("solo", "m-overflow", 999) {
        Err(MethodError::TableFull) => {}
        other => panic!("expected TableFull, got {other:?}"),
    }
    assert_eq!(
        per_provider.list_methods("solo").len(),
        MAX_METHODS_PER_PROVIDER
    );

    let before = table.len();
    for _ in 0..10_000 {
        let receipt = table
            .dispatch("prov-00", "key", &input("bulk"))
            .expect("dispatch");
        assert_eq!(receipt.handler_id, 0);
    }
    assert_eq!(table.len(), before);
}

#[test]
fn int_003_t04_failure_states_no_side_effects() {
    let mut table = MethodTable::new();
    reg(&mut table, "prov-a", "key", 7);
    let before = table.len();
    let executions = Rc::new(Cell::new(0usize));

    match table.dispatch("nope", "key", &input("h")) {
        Err(MethodError::UnknownProvider) => {}
        other => panic!("expected UnknownProvider, got {other:?}"),
    }
    match table.dispatch("prov-a", "nope", &input("h")) {
        Err(MethodError::UnknownMethod) => {}
        other => panic!("expected UnknownMethod, got {other:?}"),
    }
    for bad in ["", &"a".repeat(65), "UPPER SPACE!"] {
        match table.dispatch(bad, "key", &input("h")) {
            Err(MethodError::InvalidId) => {}
            other => panic!("expected InvalidId for {bad:?}, got {other:?}"),
        }
        match table.dispatch("prov-a", bad, &input("h")) {
            Err(MethodError::InvalidId) => {}
            other => panic!("expected InvalidId for {bad:?}, got {other:?}"),
        }
        match table.register(bad, "key", 1) {
            Err(MethodError::InvalidId) => {}
            other => panic!("expected InvalidId for {bad:?}, got {other:?}"),
        }
        match table.register("prov-a", bad, 1) {
            Err(MethodError::InvalidId) => {}
            other => panic!("expected InvalidId for {bad:?}, got {other:?}"),
        }
    }
    assert_eq!(table.len(), before);
    assert_eq!(executions.get(), 0);
}

#[test]
fn int_003_t05_no_io_no_secret_leak_no_exec_on_error() {
    let dir = tempfile::tempdir().expect("disposable test dir should exist");
    let before: Vec<String> = std::fs::read_dir(dir.path())
        .expect("dir should list")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();

    let mut table = MethodTable::new();
    reg(&mut table, "prov-a", "key", 7);
    let executions = Rc::new(Cell::new(0usize));

    let secret = "s3cr3t-handle-77-XYZ";
    let secret_input = MethodInput {
        handle: secret,
        bytes: secret.as_bytes(),
    };
    let ok = table
        .dispatch("prov-a", "key", &secret_input)
        .expect("dispatch");
    assert!(ok.accepted);

    let err = table
        .dispatch("prov-a", "missing-method", &secret_input)
        .expect_err("unknown method must fail");
    assert_eq!(err, MethodError::UnknownMethod);
    assert_eq!(executions.get(), 0);

    let mut log_capture = String::new();
    log_capture.push_str(&format!("{secret_input:?} {ok:?} {err:?}"));
    assert!(
        !log_capture.contains(secret),
        "logs must not contain handle bytes"
    );

    let after: Vec<String> = std::fs::read_dir(dir.path())
        .expect("dir should list")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(before, after);
}
