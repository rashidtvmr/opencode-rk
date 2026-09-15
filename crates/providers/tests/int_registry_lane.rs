//! INT-001 frozen tests: secret-free scoped integration registry (T01..T05).
//!
//! The implementation under test is included by path so this lane never edits
//! the shared `crates/providers/src/lib.rs` (integrator-owned).

#[path = "../src/int_registry_lane.rs"]
mod int_registry_lane;

use int_registry_lane::{
    IntegrationDesc, IntegrationRegistry, MethodKind, MethodMeta, RegistryError, ScopeToken,
    MAX_INTEGRATIONS, MAX_LABEL_LEN, MAX_METHODS_PER_INTEGRATION, MAX_METHOD_ID_LEN, MAX_NAME_LEN,
    MAX_SCOPES,
};

fn method(kind: MethodKind, id: &str, label: &str) -> MethodMeta {
    MethodMeta {
        kind,
        method_id: id.to_owned(),
        label: label.to_owned(),
    }
}

fn desc(name: &str, methods: Vec<MethodMeta>) -> IntegrationDesc {
    IntegrationDesc {
        name: name.to_owned(),
        methods,
    }
}

fn snapshot(reg: &IntegrationRegistry) -> String {
    format!("{:?}", reg.list())
}

#[test]
fn int_001_t01_happy_path_three_registrations_sorted() {
    assert_eq!(MAX_INTEGRATIONS, 64);
    assert_eq!(MAX_METHODS_PER_INTEGRATION, 8);
    assert_eq!(MAX_SCOPES, 16);
    assert_eq!(MAX_NAME_LEN, 64);
    assert_eq!(MAX_METHOD_ID_LEN, 64);
    assert_eq!(MAX_LABEL_LEN, 128);

    let mut reg = IntegrationRegistry::new();
    let scope = reg.open_scope().expect("open scope");
    let a = desc("a", vec![method(MethodKind::Key, "key", "Primary")]);
    let b = desc(
        "b",
        vec![method(MethodKind::OAuth, "oauth-main", "Main login")],
    );
    let c = desc(
        "c",
        vec![
            method(MethodKind::Key, "key", "K"),
            method(MethodKind::OAuth, "oauth-x", "X"),
        ],
    );
    reg.register(scope, a.clone()).expect("register a");
    reg.register(scope, b.clone()).expect("register b");
    reg.register(scope, c.clone()).expect("register c");

    assert_eq!(reg.get("b"), Some(&b));
    assert_eq!(reg.get("missing"), None);

    let listed = reg.list();
    assert_eq!(listed.len(), 3);
    let names: Vec<&str> = listed.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, vec!["a", "b", "c"]);
    assert_eq!(listed[1].1, &b);
}

#[test]
fn int_001_t02_override_and_reveal_on_close() {
    let mut reg = IntegrationRegistry::new();
    let s1 = reg.open_scope().expect("open s1");
    assert_eq!(s1, ScopeToken(1));
    reg.register(s1, desc("x", vec![method(MethodKind::Key, "k", "v1")]))
        .expect("register v1");
    let s2 = reg.open_scope().expect("open s2");
    assert_eq!(s2, ScopeToken(2));
    reg.register(s2, desc("x", vec![method(MethodKind::Key, "k", "v2")]))
        .expect("register v2");

    assert_eq!(
        reg.get("x").expect("x visible").methods[0].label,
        "v2"
    );
    assert_eq!(reg.close_scope(s2).expect("close s2"), 1);
    assert_eq!(
        reg.get("x").expect("x revealed").methods[0].label,
        "v1"
    );
    assert_eq!(reg.close_scope(s1).expect("close s1"), 1);
    assert_eq!(reg.get("x"), None);
}

#[test]
fn int_001_t03_validation_leaves_registry_unchanged() {
    let mut reg = IntegrationRegistry::new();
    let scope = reg.open_scope().expect("open");
    reg.register(scope, desc("dup", vec![]))
        .expect("first dup fits");
    let before = snapshot(&reg);
    assert_eq!(
        reg.register(scope, desc("dup", vec![]))
            .expect_err("duplicate in same scope"),
        RegistryError::Duplicate
    );
    assert_eq!(snapshot(&reg), before);

    let long_name = "a".repeat(65);
    for bad in ["", "-lead", "has space", "bang!", long_name.as_str()] {
        let before = snapshot(&reg);
        assert_eq!(
            reg.register(scope, desc(bad, vec![]))
                .expect_err("bad name rejected"),
            RegistryError::InvalidName,
            "name {bad:?}"
        );
        assert_eq!(snapshot(&reg), before);
    }

    let many: Vec<MethodMeta> = (0..MAX_METHODS_PER_INTEGRATION + 1)
        .map(|i| method(MethodKind::OAuth, &format!("m{i:02}"), "L"))
        .collect();
    assert_eq!(many.len(), 9);
    let before = snapshot(&reg);
    assert_eq!(
        reg.register(scope, desc("many", many))
            .expect_err("9 methods rejected"),
        RegistryError::InvalidMethod
    );
    assert_eq!(snapshot(&reg), before);

    let long_label = "l".repeat(129);
    let bad_methods: Vec<Vec<MethodMeta>> = vec![
        vec![method(MethodKind::Key, "", "L")],
        vec![method(MethodKind::Key, "-bad", "L")],
        vec![method(MethodKind::Key, "ok", long_label.as_str())],
        vec![method(MethodKind::Key, "ok", "bad\u{7f}label")],
    ];
    for methods in bad_methods {
        let before = snapshot(&reg);
        assert_eq!(
            reg.register(scope, desc("badm", methods))
                .expect_err("bad method rejected"),
            RegistryError::InvalidMethod
        );
        assert_eq!(snapshot(&reg), before);
    }

    let mut full = IntegrationRegistry::new();
    let scope = full.open_scope().expect("open");
    for i in 0..MAX_INTEGRATIONS {
        full.register(scope, desc(&format!("n{i:02}"), vec![]))
            .expect("fill to bound");
    }
    assert_eq!(full.list().len(), MAX_INTEGRATIONS);
    let before = snapshot(&full);
    assert_eq!(
        full.register(scope, desc("overflow", vec![]))
            .expect_err("over bound rejected"),
        RegistryError::Overflow
    );
    assert_eq!(full.list().len(), MAX_INTEGRATIONS);
    assert_eq!(snapshot(&full), before);
}

#[test]
fn int_001_t04_scope_discipline_is_strict_lifo() {
    assert_eq!(MAX_SCOPES, 16);
    let mut reg = IntegrationRegistry::new();
    let before = snapshot(&reg);
    assert_eq!(
        reg.close_scope(ScopeToken(9999))
            .expect_err("unknown scope"),
        RegistryError::Scope
    );
    assert_eq!(snapshot(&reg), before);

    let s1 = reg.open_scope().expect("open s1");
    reg.register(s1, desc("x", vec![])).expect("x in s1");
    let s2 = reg.open_scope().expect("open s2");
    reg.register(s2, desc("y", vec![])).expect("y in s2");
    let before = snapshot(&reg);
    assert_eq!(
        reg.close_scope(s1).expect_err("non-innermost close"),
        RegistryError::Scope
    );
    assert_eq!(snapshot(&reg), before);
    assert!(reg.get("x").is_some() && reg.get("y").is_some());

    assert_eq!(reg.close_scope(s2).expect("close s2"), 1);
    assert_eq!(reg.close_scope(s1).expect("close s1"), 1);
    assert!(reg.list().is_empty());

    let mut full = IntegrationRegistry::new();
    let mut tokens = Vec::new();
    for _ in 0..MAX_SCOPES {
        tokens.push(full.open_scope().expect("fill scopes"));
    }
    assert_eq!(
        full.open_scope().expect_err("scope cap holds"),
        RegistryError::Scope
    );
    for token in tokens.into_iter().rev() {
        full.close_scope(token).expect("lifo close");
    }
    assert!(full.list().is_empty());
}

#[test]
fn int_001_t05_no_side_effect_safety() {
    const SRC: &str = include_str!("../src/int_registry_lane.rs");
    for banned in [
        "std::process",
        "std::net",
        "tokio",
        "std::fs",
        "std::env",
        "Command",
        "TcpStream",
        "UdpSocket",
        "std::thread",
        "println!",
        "eprintln!",
    ] {
        assert!(!SRC.contains(banned), "banned handle in lane: {banned}");
    }
    for line in SRC.lines() {
        if line.contains("fn ") {
            let scrubbed = line.to_lowercase().replace("scopetoken", "");
            for word in ["secret", "token", "code"] {
                assert!(
                    !scrubbed.contains(word),
                    "banned param in signature {line:?}"
                );
            }
        }
    }

    let dir = tempfile::tempdir().expect("disposable fixture dir");
    let files = || -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir.path())
            .expect("read fixture dir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    let before = files();

    let mut reg = IntegrationRegistry::new();
    let s1 = reg.open_scope().expect("open s1");
    reg.register(
        s1,
        desc("svc-a", vec![method(MethodKind::Key, "key", "Primary")]),
    )
    .expect("register a");
    reg.register(
        s1,
        desc(
            "svc-b",
            vec![method(MethodKind::OAuth, "oauth-main", "Main login")],
        ),
    )
    .expect("register b");
    let s2 = reg.open_scope().expect("open s2");
    reg.register(
        s2,
        desc("svc-a", vec![method(MethodKind::Key, "key", "Rotated")]),
    )
    .expect("override a");
    assert_eq!(reg.list().len(), 2);
    assert_eq!(reg.close_scope(s2).expect("close s2"), 1);
    assert_eq!(reg.close_scope(s1).expect("close s1"), 2);

    let hidden = "sk-live-9f8e7d6c5b4a";
    let body = "tok-body-zz99";
    let mut logs = format!("{reg:?}");
    logs.push_str(&format!("{:?}", reg.list()));
    logs.push_str(&format!("{:?}", RegistryError::Duplicate));
    logs.push_str(&format!("{:?}", RegistryError::Overflow));
    assert!(!logs.contains(hidden), "hidden material leaked");
    assert!(!logs.contains(body), "hidden material leaked");

    let made = IntegrationDesc {
        name: String::from("svc-c"),
        methods: vec![MethodMeta {
            kind: MethodKind::OAuth,
            method_id: String::from("oauth-x"),
            label: String::from("X"),
        }],
    };
    assert_eq!(made.methods.len(), 1);

    assert_eq!(files(), before);
    assert!(files()
        .iter()
        .all(|n| !n.ends_with(".db") && !n.ends_with(".sqlite")));
}
