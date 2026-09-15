//! WEB-017 FROZEN RED: editable writing/code artifacts (pure, side-effect-free).
//! T01 happy path, T02 execution/apply safety, T03 accessibility/focus,
//! T04 bounds/cancellation, T05 version/reload fidelity.
//! `#[path]` include: lib.rs untouched, no fs/net/process in impl.
#[path = "../src/web_artifact.rs"]
mod web_artifact;

use web_artifact::{
    ArtifactKind, RunError, authorize_run, copy_text, edit_artifact, open_artifact,
    open_code, preview_artifact, redo_edit, restore_snapshot, snapshot, toggle_preview,
    undo_edit, MAX_ARTIFACT_BYTES, MAX_EXEC_OUTPUT_BYTES, MAX_PREVIEW_BYTES,
    MAX_VERSIONS, artifact_controls,
};

fn writing() -> web_artifact::Artifact {
    open_artifact(7, ArtifactKind::Writing, "Essay", "hello world").expect("open writing")
}

fn code() -> web_artifact::Artifact {
    open_code(9, "main.py", "print('hi')", "python").expect("open code")
}

#[test]
fn artifact_t01_happy_path_edit_copy_preview_preserves_original() {
    let mut a = writing();
    assert_eq!(copy_text(&a), "hello world");
    let p = preview_artifact(&a, false).expect("preview");
    assert!(!p.truncated);
    assert_eq!(p.text, "hello world");
    let seq = edit_artifact(&mut a, "hello brave world").expect("edit");
    assert_eq!(seq, 2);
    assert_eq!(copy_text(&a), "hello brave world");
    assert_eq!(a.original_body(), "hello world", "original message never mutates");
    let p2 = preview_artifact(&a, false).expect("preview after edit");
    assert_eq!(p2.text, "hello brave world");
}

#[test]
fn artifact_t02_run_apply_disabled_without_executor_and_gated_when_enabled() {
    let mut a = code();
    // No safe native executor: run/apply disabled explicitly.
    let err = authorize_run(false, true).expect_err("no executor must disable run");
    assert!(matches!(err, RunError::NoExecutor));
    assert!(format!("{err}").contains("executor"));
    // Executor present but no explicit grant: denied, zero side effects.
    let before = snapshot(&a);
    let denied = authorize_run(true, false).expect_err("grant required");
    assert!(matches!(denied, RunError::Denied));
    assert_eq!(snapshot(&a), before, "denied run must not touch artifact");
    // Enabled + explicitly granted: permit carries bounded output.
    let permit = authorize_run(true, true).expect("granted run");
    let out = permit.execute("x".repeat(MAX_EXEC_OUTPUT_BYTES + 8));
    assert!(out.text.len() <= MAX_EXEC_OUTPUT_BYTES, "exec output bounded");
    assert!(out.truncated);
    assert_eq!(copy_text(&a), "print('hi')", "execution never rewrites artifact");
    // Impl must not hide a real executor: source scan.
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/web_artifact.rs"
    ))
    .expect("read artifact source");
    for banned in [
        "TcpListener", "UdpSocket", "tokio::spawn", "std::process", "Command",
        "std::net", "std::fs", "listen(", "connect(",
    ] {
        assert!(!source.contains(banned), "no hidden executor: found {banned:?}");
    }
    for secret_word in ["secret", "token", "password"] {
        assert!(
            !source.to_lowercase().contains(secret_word),
            "zero secrets: found {secret_word:?}"
        );
    }
}

#[test]
fn artifact_t03_controls_keyboard_operable_and_focus_preserved() {
    let ctrls = artifact_controls(ArtifactKind::Code);
    assert!(ctrls.len() >= 5, "editor/preview/undo/redo/copy/run controls");
    for c in &ctrls {
        assert!(!c.label.is_empty(), "screen-reader label required");
        assert!(!c.role.is_empty(), "ARIA role required");
        assert!(!c.shortcut.is_empty(), "keyboard path required: {}", c.label);
    }
    let mut a = writing();
    a.set_selection(2, 5);
    let focus_before = a.focus_id();
    let shown = toggle_preview(&mut a);
    assert!(shown);
    assert_eq!(a.focus_id(), focus_before, "preview toggle preserves focus");
    assert_eq!(a.selection(), (2, 5), "preview toggle preserves selection");
    edit_artifact(&mut a, "hello brave world").expect("edit");
    a.set_selection(0, 5);
    undo_edit(&mut a).expect("undo");
    assert_eq!(a.selection(), (0, 5), "undo preserves selection");
    assert_eq!(a.focus_id(), focus_before, "undo preserves focus");
    redo_edit(&mut a).expect("redo");
    assert_eq!(copy_text(&a), "hello brave world");
}

#[test]
fn artifact_t04_bounds_history_preview_cancellable() {
    // Oversized document rejected.
    let big = "b".repeat(MAX_ARTIFACT_BYTES + 1);
    assert!(open_artifact(1, ArtifactKind::Writing, "t", &big).is_err());
    let mut a = writing();
    assert!(edit_artifact(&mut a, &big).is_err());
    // History bounded: many edits retain at most MAX_VERSIONS versions.
    for i in 0..(MAX_VERSIONS + 10) {
        edit_artifact(&mut a, &format!("revision {i}")).expect("bounded edit");
    }
    assert!(a.version_count() <= MAX_VERSIONS, "editor history bounded");
    // Preview bounded + cancellable.
    let p = preview_artifact(&a, false).expect("preview");
    assert!(p.text.len() <= MAX_PREVIEW_BYTES);
    assert!(preview_artifact(&a, true).is_err(), "cancel stops preview work");
    // Side-effect-free: sentinel untouched, no files created.
    let dir = tempfile::tempdir().expect("disposable fixture");
    let sentinel = dir.path().join("sentinel.txt");
    std::fs::write(&sentinel, b"sentinel-bytes").expect("sentinel");
    let before = std::fs::read(&sentinel).expect("read sentinel");
    let mut b = writing();
    edit_artifact(&mut b, "changed").expect("edit");
    undo_edit(&mut b).expect("undo");
    let _ = preview_artifact(&b, false);
    let _ = snapshot(&b);
    assert_eq!(std::fs::read(&sentinel).expect("reread"), before);
    assert_eq!(std::fs::read_dir(dir.path()).expect("list").count(), 1);
}

#[test]
fn artifact_t05_versions_explicit_and_reload_fidelity() {
    let mut a = writing();
    edit_artifact(&mut a, "draft two").expect("edit 1");
    edit_artifact(&mut a, "draft three").expect("edit 2");
    assert_eq!(a.current_seq(), 3, "edits create explicit versions");
    assert_eq!(a.original_body(), "hello world");
    let snap = snapshot(&a);
    let b = restore_snapshot(snap).expect("reload");
    assert_eq!(copy_text(&b), "draft three", "reload restores current draft");
    assert_eq!(b.original_body(), "hello world", "reload never rewrites history");
    assert_eq!(b.current_seq(), 3);
    assert_eq!(b.version_count(), a.version_count());
    // Reloaded artifact keeps editing as new versions, original intact.
    let mut b = b;
    edit_artifact(&mut b, "draft four").expect("post-reload edit");
    assert_eq!(b.current_seq(), 4);
    assert_eq!(b.original_body(), "hello world");
}
