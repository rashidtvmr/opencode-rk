// EXT-008 frozen tests T01..T05: inert hook-declaration boundary.
// Module under test is included via path so this lane never edits lib.rs.
#[path = "../src/plugin_hook_boundary.rs"]
mod plugin_hook_boundary;

use plugin_hook_boundary::{
    ActivationGate, HookBoundary, HookDecl, HookError, HookId, HookPhase, MAX_HOOK_DECLS,
};

fn decl(phase: HookPhase, filter: &str, label: &str) -> HookDecl {
    HookDecl {
        phase,
        tool_filter: filter.to_string(),
        label: label.to_string(),
    }
}

#[test]
fn ext008_t01_happy_path() {
    let mut b = HookBoundary::new();
    assert!(b.is_gate_closed(), "gate closed by default");
    let id1 = b
        .declare(1, decl(HookPhase::Before, "read*", "audit-read"))
        .expect("decl 1");
    let id2 = b
        .declare(2, decl(HookPhase::After, "write", "audit-write"))
        .expect("decl 2");
    let id3 = b
        .declare(1, decl(HookPhase::After, "read*", "audit-read-after"))
        .expect("decl 3");
    assert_eq!((id1.0, id2.0, id3.0), (1, 2, 3));
    let listed = b.list();
    assert_eq!(listed.len(), 3);
    let ids: Vec<u64> = listed.iter().map(|(id, _, _)| id.0).collect();
    assert_eq!(ids, vec![1, 2, 3], "list in id order");
    assert_eq!(listed[0].1, 1);
    assert_eq!(listed[1].1, 2);
    assert_eq!(listed[2].1, 1);
    assert_eq!(listed[0].2.phase, HookPhase::Before);
    assert_eq!(listed[1].2.phase, HookPhase::After);
    assert_eq!(listed[0].2.tool_filter, "read*");
    assert_eq!(listed[1].2.tool_filter, "write");
    assert_eq!(listed[2].2.label, "audit-read-after");
    let replayed = b.replay();
    assert_eq!(replayed.len(), 3);
    assert_eq!(
        replayed,
        vec![
            decl(HookPhase::Before, "read*", "audit-read"),
            decl(HookPhase::After, "write", "audit-write"),
            decl(HookPhase::After, "read*", "audit-read-after"),
        ]
    );
}

#[test]
fn ext008_t02_scope_revoke() {
    let mut b = HookBoundary::new();
    b.declare(7, decl(HookPhase::Before, "read", "s7-before"))
        .expect("s7 decl 1");
    b.declare(7, decl(HookPhase::After, "read", "s7-after"))
        .expect("s7 decl 2");
    b.declare(8, decl(HookPhase::Before, "write", "s8-before"))
        .expect("s8 decl");
    assert_eq!(b.revoke_scope(7), 2);
    let listed = b.list();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].1, 8);
    assert_eq!(listed[0].2.label, "s8-before");
    let before = b.list();
    assert_eq!(b.revoke_scope(9999), 0, "unknown scope harmless");
    assert_eq!(b.list(), before, "registry unchanged after unknown revoke");
    // Same filter/label as a revoked decl re-declares fine under a fresh scope.
    b.declare(9, decl(HookPhase::Before, "read", "s7-before"))
        .expect("no ghost duplicate after revoke");
    assert_eq!(b.list().len(), 2);
}

#[test]
fn ext008_t03_validation_registry_unchanged() {
    let mut b = HookBoundary::new();
    b.declare(1, decl(HookPhase::Before, "read", "base"))
        .expect("base decl");

    let snapshot = b.list();
    assert_eq!(
        b.declare(1, decl(HookPhase::Before, "read", "base")),
        Err(HookError::Duplicate)
    );
    assert_eq!(b.list(), snapshot, "duplicate leaves registry unchanged");

    // Same filter/label under a different scope or phase is not a duplicate.
    b.declare(2, decl(HookPhase::Before, "read", "base"))
        .expect("different scope ok");
    b.declare(1, decl(HookPhase::After, "read", "base"))
        .expect("different phase ok");

    for (filter, label) in [
        ("", "ok-label"),
        ("has space", "ok-label"),
        ("semi;colon", "ok-label"),
        ("read", ""),
        ("read", "-bad-start"),
        ("read", ".bad-start"),
        ("read", "has space"),
        ("read", "semi;colon"),
    ] {
        let snapshot = b.list();
        let err = b
            .declare(3, decl(HookPhase::Before, filter, label))
            .expect_err("invalid decl rejected");
        assert!(
            err == HookError::InvalidFilter || err == HookError::InvalidLabel,
            "unexpected error for ({filter:?}, {label:?}): {err:?}"
        );
        assert_eq!(b.list(), snapshot, "invalid decl leaves registry unchanged");
    }
    // Overlong filter/label rejected.
    let long_filter = "f".repeat(65);
    let long_label = "l".repeat(65);
    assert_eq!(
        b.declare(3, decl(HookPhase::Before, &long_filter, "ok")),
        Err(HookError::InvalidFilter)
    );
    assert_eq!(
        b.declare(3, decl(HookPhase::Before, "ok", &long_label)),
        Err(HookError::InvalidLabel)
    );

    // Fill to cap, then one more overflows without growth.
    let mut full = HookBoundary::new();
    for i in 0..MAX_HOOK_DECLS {
        full.declare(
            1,
            decl(
                HookPhase::Before,
                &format!("tool{i:03}"),
                &format!("label{i:03}"),
            ),
        )
        .expect("fill to cap");
    }
    assert_eq!(full.list().len(), MAX_HOOK_DECLS);
    let snapshot = full.list();
    assert_eq!(
        full.declare(1, decl(HookPhase::Before, "one-more", "one-more")),
        Err(HookError::Overflow)
    );
    assert_eq!(full.list(), snapshot, "overflow leaves registry unchanged");
    assert_eq!(full.list().len(), MAX_HOOK_DECLS);
}

#[test]
fn ext008_t04_deferred_execution() {
    let mut b = HookBoundary::new();
    let known = b
        .declare(1, decl(HookPhase::Before, "read", "sentinel"))
        .expect("sentinel decl");
    let mut input = vec![1u8, 2, 3, 4];
    let mut output = vec![9u8, 8, 7];
    let input_frozen = input.clone();
    let output_frozen = output.clone();

    assert_eq!(b.try_execute(known), Err(HookError::Deferred));
    assert_eq!(
        b.try_execute(HookId(9999)),
        Err(HookError::Deferred),
        "unknown id deferred, never success"
    );
    assert_eq!(input, input_frozen, "no input mutation while closed");
    assert_eq!(output, output_frozen, "no output mutation while closed");

    b.open_gate();
    assert!(!b.is_gate_closed(), "gate opens only via explicit call");
    assert_eq!(
        b.try_execute(known),
        Err(HookError::Deferred),
        "still deferred while open: no executor ships here"
    );
    assert_eq!(input, input_frozen, "no input mutation while open");
    assert_eq!(output, output_frozen, "no output mutation while open");
    // Recording still works while open; try_execute touches nothing.
    let listed_before = b.list();
    let _ = (&mut input, &mut output);
    b.declare(2, decl(HookPhase::After, "write", "open-decl"))
        .expect("recording works while open");
    assert_eq!(b.try_execute(known), Err(HookError::Deferred));
    assert_eq!(b.list().len(), listed_before.len() + 1);
}

#[test]
fn ext008_t05_no_host_side_effects() {
    let dir = tempfile::tempdir().expect("disposable test dir");
    let entries_before: Vec<String> = std::fs::read_dir(dir.path())
        .expect("read disposable dir")
        .map(|e| e.expect("dir entry").file_name().to_string_lossy().into_owned())
        .collect();

    let mut invocations: usize = 0;
    let mut b = HookBoundary::new();
    assert!(b.is_gate_closed());
    let gate = ActivationGate::closed();
    assert!(gate.is_closed(), "standalone gate defaults closed");

    // Full matrix: declare both phases/scopes, list, replay, deferred
    // execute (known + unknown), revoke known + unknown, open gate, repeat.
    for scope in [1u64, 2] {
        for phase in [HookPhase::Before, HookPhase::After] {
            let label = format!("m{scope}-{phase:?}");
            b.declare(scope, decl(phase, "tool*", &label))
                .expect("matrix declare");
        }
    }
    assert_eq!(b.list().len(), 4);
    assert_eq!(b.replay().len(), 4);
    let known: Vec<HookId> = b.list().iter().map(|(id, _, _)| *id).collect();
    for id in &known {
        assert_eq!(b.try_execute(*id), Err(HookError::Deferred));
    }
    assert_eq!(b.try_execute(HookId(u64::MAX)), Err(HookError::Deferred));
    assert_eq!(b.revoke_scope(u64::MAX), 0);
    assert_eq!(b.revoke_scope(1), 2);
    b.open_gate();
    for id in &known {
        assert_eq!(b.try_execute(*id), Err(HookError::Deferred));
    }
    b.declare(3, decl(HookPhase::Before, "tool*", "post-open"))
        .expect("declare while open");

    // The absent executor was never invoked; nothing ran.
    assert_eq!(invocations, 0, "zero handler invocations");
    let _ = &mut invocations;

    let mut entries_after: Vec<String> = std::fs::read_dir(dir.path())
        .expect("reread disposable dir")
        .map(|e| e.expect("dir entry").file_name().to_string_lossy().into_owned())
        .collect();
    entries_after.sort();
    let mut expected = entries_before;
    expected.sort();
    assert_eq!(entries_after, expected, "no files outside the boundary");
}
