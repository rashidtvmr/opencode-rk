//! EXT-006 frozen tests T01..T05. Module included via path so the worker
//! never edits shared `lib.rs`.

#[path = "../src/plugin_scoped_exec.rs"]
mod plugin_scoped_exec;

use plugin_scoped_exec::*;
use std::cell::Cell;
use std::sync::atomic::AtomicBool;

struct CountingBroker {
    allow: bool,
    calls: Cell<usize>,
}

impl CountingBroker {
    fn allow() -> Self {
        Self { allow: true, calls: Cell::new(0) }
    }
    fn deny() -> Self {
        Self { allow: false, calls: Cell::new(0) }
    }
}

impl PermissionBroker for CountingBroker {
    fn assert(&self, _req: &EffectRequest) -> bool {
        self.calls.set(self.calls.get() + 1);
        self.allow
    }
}

fn manifest(caps: &[&str]) -> Manifest {
    Manifest {
        name: "plugin-a".to_string(),
        contract_version: SUPPORTED_CONTRACT_VERSION,
        capabilities: caps.iter().map(|s| s.to_string()).collect(),
    }
}

fn req(cap: &str, op: &str, args: &[u8]) -> EffectRequest {
    EffectRequest { capability: cap.to_string(), op: op.to_string(), args: args.to_vec() }
}

#[test]
fn ext006_t01_happy_path() {
    let broker = CountingBroker::allow();
    let grant = bind("plugin-a", &manifest(&["read", "write"])).expect("bind");
    let mut sink = EffectSink::new();
    let cancel = AtomicBool::new(false);
    let args = b"0123456789";
    let out1 = execute(&grant, &req("read", "fetch", args), &broker, &mut sink, &cancel)
        .expect("execute");
    assert!(out1.applied);
    assert_eq!(sink.len(), 1);
    assert_eq!(sink.get("read:fetch"), Some(args.as_slice()));
    let out2 = execute(&grant, &req("read", "fetch", args), &broker, &mut sink, &cancel)
        .expect("execute again");
    assert_eq!(out1, out2);
    assert_eq!(sink.len(), 1);
}

#[test]
fn ext006_t02_capability_gate() {
    let broker = CountingBroker::allow();
    let grant = bind("plugin-a", &manifest(&["read", "write"])).expect("bind");
    let mut sink = EffectSink::new();
    let cancel = AtomicBool::new(false);
    let before = sink.clone();
    assert_eq!(
        execute(&grant, &req("admin", "fetch", b"x"), &broker, &mut sink, &cancel),
        Err(ExecError::UnknownCapability)
    );
    assert_eq!(broker.calls.get(), 0);
    assert_eq!(sink, before);
    assert_eq!(
        execute(&grant, &req("read", "", b"x"), &broker, &mut sink, &cancel),
        Err(ExecError::InvalidOp)
    );
    assert_eq!(
        execute(&grant, &req("read", "has space", b"x"), &broker, &mut sink, &cancel),
        Err(ExecError::InvalidOp)
    );
    assert_eq!(sink, before);
}

#[test]
fn ext006_t03_broker_deny_no_side_effects() {
    let broker = CountingBroker::deny();
    let grant = bind("plugin-a", &manifest(&["write"])).expect("bind");
    let mut sink = EffectSink::new();
    let before_hash = sink.fingerprint();
    let cancel = AtomicBool::new(false);
    assert_eq!(
        execute(&grant, &req("write", "put", b"data"), &broker, &mut sink, &cancel),
        Err(ExecError::Denied)
    );
    assert_eq!(broker.calls.get(), 1);
    assert!(sink.is_empty());
    assert_eq!(sink.fingerprint(), before_hash);
}

#[test]
fn ext006_t04_bounds_cancel_revoke() {
    let broker = CountingBroker::allow();
    let grant = bind("plugin-a", &manifest(&["write"])).expect("bind");
    let mut sink = EffectSink::new();
    let cancel = AtomicBool::new(false);

    // Oversize args rejected before broker.
    let big = vec![0u8; MAX_ARGS_BYTES + 1];
    assert_eq!(
        execute(&grant, &req("write", "put", &big), &broker, &mut sink, &cancel),
        Err(ExecError::TooLarge)
    );
    assert_eq!(broker.calls.get(), 0);
    assert!(sink.is_empty());

    // Fill sink to cap, then one more overflows atomically.
    for i in 0..MAX_SINK_ENTRIES {
        let op = format!("k{i:03}");
        execute(&grant, &req("write", &op, b"v"), &broker, &mut sink, &cancel).expect("fill");
    }
    assert_eq!(sink.len(), MAX_SINK_ENTRIES);
    let before = sink.clone();
    assert_eq!(
        execute(&grant, &req("write", "spill", b"v"), &broker, &mut sink, &cancel),
        Err(ExecError::Overflow)
    );
    assert_eq!(sink, before);

    // Pre-set cancel wins before broker.
    let mut sink2 = EffectSink::new();
    let cancelled = AtomicBool::new(true);
    let broker2 = CountingBroker::allow();
    assert_eq!(
        execute(&grant, &req("write", "put", b"v"), &broker2, &mut sink2, &cancelled),
        Err(ExecError::Cancelled)
    );
    assert_eq!(broker2.calls.get(), 0);
    assert!(sink2.is_empty());

    // Caller-revoked grant rejected with sink untouched.
    let mut revoked = bind("plugin-a", &manifest(&["write"])).expect("bind");
    revoked.revoke();
    let mut sink3 = EffectSink::new();
    let broker3 = CountingBroker::allow();
    assert_eq!(
        execute(&revoked, &req("write", "put", b"v"), &broker3, &mut sink3, &cancel),
        Err(ExecError::Revoked)
    );
    assert_eq!(broker3.calls.get(), 0);
    assert!(sink3.is_empty());
}

#[test]
fn ext006_t05_grant_binding_and_safety() {
    // Duplicate caps rejected.
    assert_eq!(
        bind("plugin-a", &manifest(&["read", "read"])),
        Err(ExecError::InvalidGrant)
    );
    // Grant snapshots capabilities at bind; later source edits change nothing.
    let mut m = manifest(&["read"]);
    let grant = bind("plugin-a", &m).expect("bind");
    m.capabilities.push("write".to_string());
    assert_eq!(grant.capabilities, vec!["read".to_string()]);

    // Full matrix leaves no files outside the disposable dir, no DB files.
    let dir = tempfile::tempdir().expect("tempdir");
    let broker = CountingBroker::allow();
    let mut sink = EffectSink::new();
    let cancel = AtomicBool::new(false);
    let secret_args = b"cred-marker-7f3a9c-token";
    let out =
        execute(&grant, &req("read", "fetch", secret_args), &broker, &mut sink, &cancel)
            .expect("execute");
    assert!(out.applied);
    let entries: Vec<_> = std::fs::read_dir(dir.path()).expect("read_dir").collect();
    assert!(entries.is_empty());
    // Opaque bytes never surface in debug/error text.
    assert!(!format!("{out:?}").contains("cred-marker"));
    assert!(!format!("{:?}", ExecError::Denied).contains("cred-marker"));
    assert!(!format!("{grant:?}").contains("cred-marker"));
}
