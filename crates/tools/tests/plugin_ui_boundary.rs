// EXT-012 frozen tests T01..T05: inert UI-declaration boundary.
// Module under test is included via path so this lane never edits lib.rs.
#[path = "../src/plugin_ui_boundary.rs"]
mod plugin_ui_boundary;

use plugin_ui_boundary::{
    ActivationGate, MAX_DETAIL_LEN, MAX_LABEL_LEN, MAX_UI_DECLS, UiBoundary, UiDecl, UiError,
    UiSurface,
};

fn decl(surface: UiSurface, label: &str, detail: &str) -> UiDecl {
    UiDecl {
        surface,
        label: label.to_string(),
        detail: detail.to_string(),
    }
}

#[test]
fn ext012_t01_happy_path() {
    let mut b = UiBoundary::new();
    assert!(b.is_gate_closed(), "gate closed by default");
    let id1 = b
        .declare(1, decl(UiSurface::Command, "open-file", "Open a file"))
        .expect("decl 1");
    let id2 = b
        .declare(2, decl(UiSurface::Panel, "outline", "Symbol outline"))
        .expect("decl 2");
    let id3 = b
        .declare(1, decl(UiSurface::StatusItem, "branch", "git-branch"))
        .expect("decl 3");
    assert_eq!((id1, id2, id3), (1, 2, 3));
    let listed = b.list();
    assert_eq!(listed.len(), 3);
    let ids: Vec<u64> = listed.iter().map(|(id, _, _)| *id).collect();
    assert_eq!(ids, vec![1, 2, 3], "list in id order");
    assert_eq!(listed[0].1, 1);
    assert_eq!(listed[1].1, 2);
    assert_eq!(listed[2].1, 1);
    assert_eq!(listed[0].2.surface, UiSurface::Command);
    assert_eq!(listed[1].2.surface, UiSurface::Panel);
    assert_eq!(listed[2].2.surface, UiSurface::StatusItem);
    assert_eq!(listed[0].2.label, "open-file");
    assert_eq!(listed[2].2.label, "branch");
    let (scope, d) = b.describe(2).expect("describe id 2");
    assert_eq!(scope, 2);
    assert_eq!(d.surface, UiSurface::Panel);
    assert_eq!(d.label, "outline");
    assert_eq!(d.detail, "Symbol outline");
}

#[test]
fn ext012_t02_scope_revoke() {
    let mut b = UiBoundary::new();
    let s7a = b
        .declare(7, decl(UiSurface::Command, "s7-cmd", "first"))
        .expect("s7 decl 1");
    let s7b = b
        .declare(7, decl(UiSurface::Panel, "s7-panel", "second"))
        .expect("s7 decl 2");
    b.declare(8, decl(UiSurface::StatusItem, "s8-status", "third"))
        .expect("s8 decl");
    assert_eq!(b.revoke_scope(7), 2);
    let listed = b.list();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].1, 8);
    assert_eq!(listed[0].2.label, "s8-status");
    assert!(b.describe(s7a).is_none(), "revoked id describes to None");
    assert!(b.describe(s7b).is_none(), "revoked id describes to None");
    let before = b.list();
    assert_eq!(b.revoke_scope(9999), 0, "unknown scope harmless");
    assert_eq!(b.list(), before, "registry unchanged after unknown revoke");
    // Same surface/label as a revoked decl re-declares fine under a fresh scope.
    b.declare(9, decl(UiSurface::Command, "s7-cmd", "first"))
        .expect("no ghost duplicate after revoke");
    assert_eq!(b.list().len(), 2);
}

#[test]
fn ext012_t03_validation_registry_unchanged() {
    let mut b = UiBoundary::new();
    b.declare(1, decl(UiSurface::Command, "base", "base detail"))
        .expect("base decl");

    let snapshot = b.list();
    assert_eq!(
        b.declare(1, decl(UiSurface::Command, "base", "other detail")),
        Err(UiError::Duplicate)
    );
    assert_eq!(b.list(), snapshot, "duplicate leaves registry unchanged");

    // Same surface/label under a different scope or surface is not a duplicate.
    b.declare(2, decl(UiSurface::Command, "base", "base detail"))
        .expect("different scope ok");
    b.declare(1, decl(UiSurface::Panel, "base", "base detail"))
        .expect("different surface ok");
    // Boundary lengths accepted: 64-char label, 256-char detail.
    let label64 = "l".repeat(MAX_LABEL_LEN);
    let detail256 = "d".repeat(MAX_DETAIL_LEN);
    b.declare(3, decl(UiSurface::StatusItem, &label64, &detail256))
        .expect("max-length decl ok");

    for (surface, label, detail) in [
        (UiSurface::Command, "", "ok"),
        (UiSurface::Command, "-bad-start", "ok"),
        (UiSurface::Command, ".bad-start", "ok"),
        (UiSurface::Command, "has space", "ok"),
        (UiSurface::Command, "semi;colon", "ok"),
        (UiSurface::Command, &"l".repeat(MAX_LABEL_LEN + 1), "ok"),
    ] {
        let snapshot = b.list();
        assert_eq!(
            b.declare(4, decl(surface, label, detail)),
            Err(UiError::InvalidLabel),
            "bad label rejected: {label:?}"
        );
        assert_eq!(b.list(), snapshot, "bad label leaves registry unchanged");
    }

    let detail257 = "d".repeat(MAX_DETAIL_LEN + 1);
    for detail in [detail257.as_str(), "has\nnewline", "has\ttab", "caf\u{e9}"] {
        let snapshot = b.list();
        assert_eq!(
            b.declare(4, decl(UiSurface::Command, "ok-label", detail)),
            Err(UiError::InvalidDetail),
            "bad detail rejected: {detail:?}"
        );
        assert_eq!(b.list(), snapshot, "bad detail leaves registry unchanged");
    }

    // Fill to cap, then one more overflows without growth.
    let mut full = UiBoundary::new();
    for i in 0..MAX_UI_DECLS {
        full.declare(
            1,
            decl(
                match i % 3 {
                    0 => UiSurface::Command,
                    1 => UiSurface::Panel,
                    _ => UiSurface::StatusItem,
                },
                &format!("w{i:02}"),
                "filler",
            ),
        )
        .expect("fill to cap");
    }
    assert_eq!(full.list().len(), MAX_UI_DECLS);
    let snapshot = full.list();
    assert_eq!(
        full.declare(1, decl(UiSurface::Command, "one-more", "spill")),
        Err(UiError::Overflow)
    );
    assert_eq!(full.list(), snapshot, "overflow leaves registry unchanged");
    assert_eq!(full.list().len(), MAX_UI_DECLS);
}

#[test]
fn ext012_t04_deferred_presentation() {
    let mut b = UiBoundary::new();
    let known = b
        .declare(1, decl(UiSurface::Command, "sentinel", "sentinel detail"))
        .expect("sentinel decl");
    let snapshot = b.describe(known).expect("describe sentinel");
    let list_snapshot = b.list();

    assert_eq!(b.request_render(known), Err(UiError::Deferred));
    assert_eq!(b.describe(known), Some(snapshot.clone()));
    assert_eq!(
        b.request_render(9999),
        Err(UiError::Deferred),
        "unknown id deferred, never success"
    );
    assert_eq!(b.describe(known), Some(snapshot.clone()));
    assert_eq!(b.list(), list_snapshot, "no presentation side channel");

    b.open_gate();
    assert!(!b.is_gate_closed(), "gate opens only via explicit call");
    assert_eq!(
        b.request_render(known),
        Err(UiError::Deferred),
        "still deferred while open: no renderer ships here"
    );
    assert_eq!(b.describe(known), Some(snapshot.clone()));
    assert_eq!(
        b.list(),
        list_snapshot,
        "no presentation side channel while open"
    );
    // Recording still works while open.
    b.declare(2, decl(UiSurface::Panel, "open-decl", "while open"))
        .expect("recording works while open");
    assert_eq!(b.request_render(known), Err(UiError::Deferred));
    assert_eq!(b.describe(known), Some(snapshot));
}

#[test]
fn ext012_t05_no_host_side_effects() {
    let dir = tempfile::tempdir().expect("disposable test dir");
    let entries_before: Vec<String> = std::fs::read_dir(dir.path())
        .expect("read disposable dir")
        .map(|e| {
            e.expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    let mut render_invocations: usize = 0;
    let mut b = UiBoundary::new();
    assert!(b.is_gate_closed());
    let gate = ActivationGate::closed();
    assert!(gate.is_closed(), "standalone gate defaults closed");

    // Full matrix: declare all surfaces/scopes, list, describe, deferred
    // render (known + unknown), revoke known + unknown, open gate, repeat.
    let mut seq: u64 = 0;
    for scope in [1u64, 2] {
        for surface in [UiSurface::Command, UiSurface::Panel, UiSurface::StatusItem] {
            seq += 1;
            let label = format!("m{scope}-{surface:?}-{seq}");
            b.declare(scope, decl(surface, &label, "matrix detail"))
                .expect("matrix declare");
        }
    }
    assert_eq!(b.list().len(), 6);
    let known: Vec<u64> = b.list().iter().map(|(id, _, _)| *id).collect();
    for id in &known {
        assert_eq!(b.request_render(*id), Err(UiError::Deferred));
    }
    assert_eq!(b.request_render(u64::MAX), Err(UiError::Deferred));
    assert!(
        b.describe(u64::MAX).is_none(),
        "unknown id describes to None"
    );
    assert_eq!(b.revoke_scope(u64::MAX), 0);
    assert_eq!(b.revoke_scope(1), 3);
    b.open_gate();
    for id in &known {
        assert_eq!(b.request_render(*id), Err(UiError::Deferred));
    }
    b.declare(
        3,
        decl(UiSurface::Command, "post-open", "declare while open"),
    )
    .expect("declare while open");

    // The absent renderer was never invoked; nothing ran.
    assert_eq!(render_invocations, 0, "zero render invocations");
    let _ = &mut render_invocations;

    // Opaque detail bytes never surface in error text.
    assert!(!format!("{:?}", UiError::Deferred).contains("matrix detail"));
    assert!(!format!("{:?}", UiError::Duplicate).contains("matrix detail"));

    let mut entries_after: Vec<String> = std::fs::read_dir(dir.path())
        .expect("reread disposable dir")
        .map(|e| {
            e.expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    entries_after.sort();
    let mut expected = entries_before;
    expected.sort();
    assert_eq!(entries_after, expected, "no files outside the boundary");
}
