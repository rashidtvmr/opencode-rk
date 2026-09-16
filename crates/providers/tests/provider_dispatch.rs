//! INT-003 provider-method dispatch boundary tests (frozen T01..T05).
//!
//! The implementation under test is included by path so the integrator can
//! wire `crates/providers/src/provider_dispatch.rs` into `lib.rs` without
//! this lane touching shared files.

#[path = "../src/provider_dispatch.rs"]
mod provider_dispatch;

use provider_dispatch::{
    MethodError, MethodInput, MethodTable, ProviderMethodKind, MAX_INPUT_BYTES,
    MAX_METHODS_PER_PROVIDER, MAX_PROVIDERS,
};

fn input(bytes: &[u8]) -> MethodInput<'_> {
    MethodInput::new(bytes)
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn int_003_t01_register_dispatch_happy_path() {
    let mut table = MethodTable::new();
    table
        .register("prov-a", "key", 1)
        .expect("register prov-a/key");
    table
        .register("prov-a", "oauth", 2)
        .expect("register prov-a/oauth");
    table
        .register("prov-b", "key", 3)
        .expect("register prov-b/key");
    table
        .register("prov-b", "oauth", 4)
        .expect("register prov-b/oauth");

    assert_eq!(table.providers().len(), 2);
    assert_eq!(table.list_methods("prov-a").len(), 2);
    assert_eq!(table.list_methods("prov-b").len(), 2);

    let kind = ProviderMethodKind::named("prov-a", "key").expect("named kind");
    match kind {
        ProviderMethodKind::ProviderNamed { provider, method } => {
            assert_eq!(provider.as_str(), "prov-a");
            assert_eq!(method.as_str(), "key");
        }
        other => panic!("expected ProviderNamed, got {other:?}"),
    }

    let receipt = table
        .dispatch("prov-a", "key", &input(b"handle-a"))
        .expect("dispatch prov-a/key");
    assert_eq!(receipt.handler_id, 1);
    assert!(receipt.accepted);

    let receipt = table
        .dispatch("prov-b", "oauth", &input(b"handle-b"))
        .expect("dispatch prov-b/oauth");
    assert_eq!(receipt.handler_id, 4);
    assert!(receipt.accepted);
}

#[test]
fn int_003_t02_determinism_ordering_replace() {
    let mut t1 = MethodTable::new();
    t1.register("zzz", "m", 1).expect("t1 zzz");
    t1.register("aaa", "m", 2).expect("t1 aaa");
    t1.register("prov", "b", 3).expect("t1 prov/b");
    t1.register("prov", "a", 4).expect("t1 prov/a");

    let mut t2 = MethodTable::new();
    t2.register("aaa", "m", 2).expect("t2 aaa");
    t2.register("zzz", "m", 1).expect("t2 zzz");
    t2.register("prov", "a", 4).expect("t2 prov/a");
    t2.register("prov", "b", 3).expect("t2 prov/b");

    assert_eq!(t1.providers(), t2.providers());
    assert_eq!(t1.list_methods("prov"), t2.list_methods("prov"));

    let providers = t1.providers();
    assert!(
        providers.windows(2).all(|w| w[0] <= w[1]),
        "providers sorted: {providers:?}"
    );
    let methods = t1.list_methods("prov");
    assert!(
        methods.windows(2).all(|w| w[0] <= w[1]),
        "methods sorted: {methods:?}"
    );

    let before = t1.len();
    t1.register("prov", "a", 99)
        .expect("duplicate replaces in place");
    assert_eq!(t1.len(), before);
    let receipt = t1
        .dispatch("prov", "a", &input(b"h"))
        .expect("dispatch hits replaced handler");
    assert_eq!(receipt.handler_id, 99);
    assert!(receipt.accepted);
}

#[test]
fn int_003_t03_caps_hold() {
    assert_eq!(MAX_PROVIDERS, 64);
    assert_eq!(MAX_METHODS_PER_PROVIDER, 16);

    let mut table = MethodTable::new();
    for i in 0..MAX_PROVIDERS {
        table
            .register(&format!("p-{i:02}"), "m", i as u64)
            .expect("fill providers");
    }
    assert_eq!(table.providers().len(), MAX_PROVIDERS);
    assert_eq!(table.len(), MAX_PROVIDERS);
    match table.register("overflow", "m", 999) {
        Err(MethodError::TableFull) => {}
        other => panic!("expected TableFull, got {other:?}"),
    }
    assert_eq!(table.len(), MAX_PROVIDERS);
    assert_eq!(table.providers().len(), MAX_PROVIDERS);

    let mut solo = MethodTable::new();
    for i in 0..MAX_METHODS_PER_PROVIDER {
        solo.register("solo", &format!("m-{i:02}"), i as u64)
            .expect("fill methods");
    }
    assert_eq!(solo.list_methods("solo").len(), MAX_METHODS_PER_PROVIDER);
    match solo.register("solo", "m-extra", 999) {
        Err(MethodError::TableFull) => {}
        other => panic!("expected TableFull, got {other:?}"),
    }
    assert_eq!(solo.len(), MAX_METHODS_PER_PROVIDER);

    let before = table.len();
    for _ in 0..10_000 {
        let receipt = table
            .dispatch("p-00", "m", &input(b"h"))
            .expect("bulk dispatch");
        assert_eq!(receipt.handler_id, 0);
    }
    assert_eq!(table.len(), before);
}

#[test]
fn int_003_t04_failure_states() {
    let mut table = MethodTable::new();
    table.register("prov", "key", 7).expect("seed");
    let before = table.len();
    let mut executions = 0u32;

    match table.dispatch("nope", "key", &input(b"h")) {
        Err(MethodError::UnknownProvider) => {}
        Ok(r) => {
            executions += 1;
            panic!("expected UnknownProvider, dispatched {r:?}");
        }
        other => panic!("expected UnknownProvider, got {other:?}"),
    }
    match table.dispatch("prov", "nope", &input(b"h")) {
        Err(MethodError::UnknownMethod) => {}
        Ok(r) => {
            executions += 1;
            panic!("expected UnknownMethod, dispatched {r:?}");
        }
        other => panic!("expected UnknownMethod, got {other:?}"),
    }

    let long = "a".repeat(65);
    let bad_ids: [&str; 3] = ["", &long, "UPPER SPACE!"];
    for bad in bad_ids {
        match table.register(bad, "key", 1) {
            Err(MethodError::InvalidId) => {}
            other => panic!("expected InvalidId for provider {bad:?}, got {other:?}"),
        }
        match table.register("prov", bad, 1) {
            Err(MethodError::InvalidId) => {}
            other => panic!("expected InvalidId for method {bad:?}, got {other:?}"),
        }
        match table.dispatch(bad, "key", &input(b"h")) {
            Err(MethodError::InvalidId) => {}
            other => panic!("expected InvalidId dispatch provider {bad:?}, got {other:?}"),
        }
        match table.dispatch("prov", bad, &input(b"h")) {
            Err(MethodError::InvalidId) => {}
            other => panic!("expected InvalidId dispatch method {bad:?}, got {other:?}"),
        }
    }

    assert_eq!(table.len(), before);
    assert_eq!(executions, 0);
}

#[test]
fn int_003_t05_no_side_effects_safety() {
    assert_eq!(MAX_INPUT_BYTES, 4096);

    let dir = std::env::temp_dir().join(format!("int003-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("fixture dir");
    let snapshot = || -> Vec<std::ffi::OsString> {
        std::fs::read_dir(&dir)
            .expect("read fixture dir")
            .map(|e| e.expect("entry").file_name())
            .collect()
    };
    let before = snapshot();

    let mut table = MethodTable::new();
    table.register("prov", "key", 1).expect("seed");
    let secret: &[u8] = b"opaque-handle-xyz-789";
    let receipt = table
        .dispatch("prov", "key", &input(secret))
        .expect("dispatch");
    assert!(receipt.accepted);
    let rendered = format!("{receipt:?}");
    assert!(
        !contains_bytes(rendered.as_bytes(), secret),
        "receipt must not carry handle bytes"
    );

    let mut executed = 0u32;
    match table.dispatch("prov", "missing", &input(secret)) {
        Err(MethodError::UnknownMethod) => {}
        Ok(r) => {
            executed += 1;
            panic!("expected UnknownMethod, dispatched {r:?}");
        }
        other => panic!("expected UnknownMethod, got {other:?}"),
    }
    assert_eq!(executed, 0);
    let err = table
        .dispatch("prov", "missing", &input(secret))
        .expect_err("still unknown");
    assert!(
        !contains_bytes(err.to_string().as_bytes(), secret),
        "error must not carry handle bytes"
    );

    assert_eq!(snapshot(), before);
    std::fs::remove_dir_all(&dir).ok();
}
