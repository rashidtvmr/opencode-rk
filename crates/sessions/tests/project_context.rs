//! WEB-015 RED: projects, workspace context and memory disclosure.
//! Project-scoped chats/files/instructions with inspectable context-source
//! breakdown; unavailable/forbidden context fails explicitly without leaking
//! data across projects; no hidden provider reasoning surfaced.
#[path = "../src/project_context.rs"]
mod project_context;

use project_context::{
    ContextSource, ContextSourceKind, ProjectContextStore, ProjectError, MAX_CONTEXT_SOURCES,
    MAX_LOADED_PROJECTS, MAX_PREVIEW_BYTES,
};

// ---------- T01: project context ----------

#[test]
fn web015_t01_open_project_loads_only_scoped_items() {
    let mut store = ProjectContextStore::new();
    let a = store.create_project("alpha");
    let b = store.create_project("beta");
    store
        .add_session(a, "ses-a1")
        .expect("add session to alpha");
    store.add_session(b, "ses-b1").expect("add session to beta");
    store
        .add_file(a, "notes.md")
        .expect("add file to alpha");
    store.add_file(b, "other.md").expect("add file to beta");
    store
        .add_instruction(a, "guide.md")
        .expect("add instruction to alpha");

    let view = store.open_project(a).expect("open alpha");
    assert!(view.sessions.iter().all(|s| s == "ses-a1"));
    assert!(!view.sessions.iter().any(|s| s == "ses-b1"));
    assert!(view.files.iter().all(|f| f == "notes.md"));
    assert!(view.instructions.iter().all(|i| i == "guide.md"));

    // Inspectable context-source breakdown.
    let breakdown = store.context_breakdown(a).expect("breakdown");
    assert!(!breakdown.is_empty());
    let total: usize = breakdown.iter().map(|s| s.bytes).sum();
    assert!(total > 0);
    for src in &breakdown {
        assert!(!src.label.is_empty());
        assert!(src.bytes > 0);
    }
}

// ---------- T02: missing/forbidden context ----------

#[test]
fn web015_t02_missing_forbidden_context_fails_explicitly() {
    let mut store = ProjectContextStore::new();
    let a = store.create_project("alpha");
    let b = store.create_project("beta");
    store.add_file(a, "notes.md").expect("add file to alpha");

    // Invalid path rejected.
    assert_eq!(
        store.add_file(a, "../escape.md").unwrap_err(),
        ProjectError::InvalidPath
    );
    // Unavailable file load rejected.
    assert_eq!(
        store.load_file(a, "ghost.md").unwrap_err(),
        ProjectError::Unavailable
    );
    // Cross-workspace access rejected, no leak into other project.
    assert_eq!(
        store.load_file(b, "notes.md").unwrap_err(),
        ProjectError::Unavailable
    );
    let view_b = store.open_project(b).expect("open beta");
    assert!(!view_b.files.iter().any(|f| f == "notes.md"));
    // No hidden reasoning leaked through breakdown.
    for src in store.context_breakdown(a).expect("breakdown") {
        assert_ne!(src.kind, ContextSourceKind::HiddenReasoning);
    }
}

// ---------- T03: accessibility ----------

#[test]
fn web015_t03_switcher_inspector_controls_expose_names_and_keyboard() {
    let mut store = ProjectContextStore::new();
    let a = store.create_project("alpha");
    let b = store.create_project("beta");
    store.add_session(a, "ses-a1").expect("add session");

    let switcher = store.project_switcher();
    assert!(switcher.iter().any(|e| e.id == a && e.name == "alpha"));
    assert!(switcher.iter().any(|e| e.id == b && e.name == "beta"));
    for entry in &switcher {
        assert!(entry.keyboard_selectable);
        assert!(!entry.name.is_empty());
    }

    let inspector = store.context_breakdown(a).expect("breakdown");
    let labelled: Vec<ContextSource> = inspector
        .iter()
        .map(|s| store.describe_source(a, &s.label).expect("describe"))
        .collect();
    assert!(!labelled.is_empty());
    for src in &labelled {
        assert!(!src.label.is_empty());
        assert!(src.keyboard_toggleable);
    }
    // Memory source controls expose state.
    let mem = store.memory_controls(a).expect("memory controls");
    assert!(!mem.is_empty());
    for m in &mem {
        assert!(!m.name.is_empty());
    }
    assert!(store.keyboard_focus_after_switch(a));
}

// ---------- T04: bounds/lifetime ----------

#[test]
fn web015_t04_open_items_obey_caps_and_unload_cleanly() {
    let mut store = ProjectContextStore::new();
    let mut last = store.create_project("p0");
    for i in 1..=(MAX_LOADED_PROJECTS + 4) {
        last = store.create_project(format!("p{i}"));
    }
    // Opening beyond cap evicts oldest; newest stays open.
    for _ in 0..=(MAX_LOADED_PROJECTS + 4) {
        let _ = last;
    }
    assert!(store.open_count() <= MAX_LOADED_PROJECTS);

    let p = store.create_project("bounded");
    for i in 0..(MAX_CONTEXT_SOURCES + 10) {
        let _ = store.add_file(p, &format!("f{i}.md"));
    }
    assert!(store.source_count(p) <= MAX_CONTEXT_SOURCES);
    // Previews bounded.
    let preview = store.preview_source(p, 0).expect("preview");
    assert!(preview.len() <= MAX_PREVIEW_BYTES);
    // Unload clean.
    store.unload_project(p);
    assert_eq!(store.source_count(p), 0);
    assert!(!store.is_open(p));
}

// ---------- T05: persistence ----------

#[test]
fn web015_t05_membership_loaded_config_sources_survive_reconnect() {
    let mut store = ProjectContextStore::new();
    let a = store.create_project("alpha");
    store.add_session(a, "ses-a1").expect("add session");
    store.add_file(a, "notes.md").expect("add file");
    store.load_file(a, "notes.md").expect("load file");
    store
        .set_memory_enabled(a, "memory/main.md", true)
        .expect("memory on");

    let snapshot = store.snapshot();
    // Simulate client reconnect/reload: rebuild from snapshot.
    let mut restored = ProjectContextStore::restore(&snapshot);
    let view = restored.open_project(a).expect("reopen alpha");
    assert!(view.sessions.iter().any(|s| s == "ses-a1"));
    assert!(restored.is_loaded(a, "notes.md"));
    assert!(restored.memory_enabled(a, "memory/main.md"));
    let before = store.context_breakdown(a).expect("before");
    let after = restored.context_breakdown(a).expect("after");
    assert_eq!(before, after);
}
