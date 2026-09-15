use opencode_rk_sessions::tui_state::{
    decide_composer_key, footer_hints, keybinding_help, keyboard_fallback, status_click,
    validate_memory_path, Composer, ComposerError, ComposerKey, KeyHint, MemoryFile, MemoryViewer,
    ModelEntry, SourceUsage, StatusAction, StatusItem, SubmitKeymap, MAX_QUEUED,
};

// UI-014: Enter=submit, Shift+Enter/Ctrl+J=newline, send enablement,
// draft survives interrupt, bounded queue overflow.
#[test]
fn ui_014_composer_submit_vs_newline() {
    assert_eq!(
        decide_composer_key(ComposerKey::Enter, SubmitKeymap::Enter),
        opencode_rk_sessions::tui_state::ComposerAction::Submit
    );
    assert_eq!(
        decide_composer_key(ComposerKey::ShiftEnter, SubmitKeymap::Enter),
        opencode_rk_sessions::tui_state::ComposerAction::Newline
    );
    assert_eq!(
        decide_composer_key(ComposerKey::CtrlJ, SubmitKeymap::Enter),
        opencode_rk_sessions::tui_state::ComposerAction::Newline
    );
    assert_eq!(
        decide_composer_key(ComposerKey::CtrlJ, SubmitKeymap::CtrlJ),
        opencode_rk_sessions::tui_state::ComposerAction::Submit
    );

    let mut c = Composer::new();
    assert!(!c.can_send());
    c.set_draft("hello").unwrap();
    assert!(c.can_send());
    // idle submit sends immediately
    let sent = c.submit().unwrap();
    assert!(matches!(
        sent,
        opencode_rk_sessions::tui_state::SubmitOutcome::Sent(ref s) if s == "hello"
    ));
    // busy submit queues; draft text preserved on interrupt
    c.set_draft("while busy").unwrap();
    let queued = c.submit().unwrap();
    assert!(matches!(
        queued,
        opencode_rk_sessions::tui_state::SubmitOutcome::Queued
    ));
    c.set_draft("draft kept").unwrap();
    c.interrupt();
    assert_eq!(c.draft(), "draft kept");
    assert!(!c.is_busy());

    // fill queue to bound then overflow
    let mut c2 = Composer::new();
    c2.set_draft("first").unwrap();
    let _ = c2.submit().unwrap(); // becomes busy
    for i in 0..MAX_QUEUED {
        c2.set_draft(format!("q{i}")).unwrap();
        assert!(c2.submit().is_ok());
    }
    c2.set_draft("overflow").unwrap();
    assert!(matches!(c2.submit(), Err(ComposerError::QueueFull { .. })));
}

// UI-015: model click opens provider-aware switcher, context click opens
// detail, keyboard fallbacks exist.
#[test]
fn ui_015_statusbar_switcher_fallback() {
    assert_eq!(
        status_click(StatusItem::Model),
        StatusAction::OpenModelSwitcher
    );
    assert_eq!(
        status_click(StatusItem::Context),
        StatusAction::OpenContextDetail
    );
    let models = vec![
        ModelEntry::new("a", "acme", "low"),
        ModelEntry::new("b", "other", "high"),
        ModelEntry::new("c", "acme", "high"),
    ];
    let (filtered, truncated) = opencode_rk_sessions::tui_state::filter_models(&models, "acme");
    assert!(!truncated);
    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().all(|m| m.provider == "acme"));
    // keyboard fallback equivalents
    assert_eq!(
        keyboard_fallback("ctrl-p"),
        Some(StatusAction::OpenModelSwitcher)
    );
    assert_eq!(
        keyboard_fallback("ctrl-t"),
        Some(StatusAction::OpenContextDetail)
    );
    assert_eq!(keyboard_fallback("ctrl-z"), None);
}

// UI-016: per-source breakdown sorted largest-first with truncation marker.
#[test]
fn ui_016_context_breakdown_sorted() {
    let sources = vec![
        SourceUsage::new("small", 10, false),
        SourceUsage::new("big", 900, true),
        SourceUsage::new("mid", 100, false),
    ];
    let view = opencode_rk_sessions::tui_state::context_breakdown(sources);
    assert!(!view.truncated);
    let tokens: Vec<u64> = view.entries.iter().map(|s| s.tokens).collect();
    assert_eq!(tokens, vec![900, 100, 10]);
    assert!(view.entries[0].cached);

    // over bound => explicit truncated flag, never silent
    let many: Vec<SourceUsage> = (0..300)
        .map(|i| SourceUsage::new(format!("s{i}"), i as u64, false))
        .collect();
    let view2 = opencode_rk_sessions::tui_state::context_breakdown(many);
    assert!(view2.truncated);
    assert_eq!(
        view2.entries.len(),
        opencode_rk_sessions::tui_state::MAX_SOURCES
    );
    // still sorted largest-first after truncation
    assert!(view2.entries.windows(2).all(|w| w[0].tokens >= w[1].tokens));
}

// UI-017: memory list, unload cache-cost warning, reload path validation.
#[test]
fn ui_017_memory_unload_warning() {
    let files = vec![
        MemoryFile::new("a.md", 100, 25, true),
        MemoryFile::new("b.md", 50, 10, false),
    ];
    let mut viewer = MemoryViewer::new(files);
    assert_eq!(viewer.list().len(), 2);
    let warning = viewer.unload("a.md").unwrap();
    assert!(warning.contains("a.md"), "warning names file: {warning}");
    assert!(
        warning.to_lowercase().contains("cache"),
        "cached unload warns cache cost: {warning}"
    );
    let plain = viewer.unload("b.md").unwrap();
    assert!(plain.contains("b.md"));
    assert!(viewer.unload("missing.md").is_err());
    // reload validates path
    assert!(validate_memory_path("").is_err());
    assert!(validate_memory_path("../evil.md").is_err());
    viewer
        .reload(MemoryFile::new("c.md", 10, 5, false))
        .unwrap();
    assert!(viewer.list().iter().any(|f| f.path == "c.md"));
}

// UI-018: footer hints + configurable submit keymap help text.
#[test]
fn ui_018_keybinding_help() {
    let hints_enter: Vec<KeyHint> = footer_hints(SubmitKeymap::Enter);
    assert!(!hints_enter.is_empty());
    assert!(hints_enter.len() <= opencode_rk_sessions::tui_state::MAX_HINTS);
    let help_enter = keybinding_help(SubmitKeymap::Enter);
    assert!(
        help_enter.contains("Enter"),
        "help names Enter: {help_enter}"
    );
    assert!(
        help_enter.contains("Ctrl+J"),
        "help names Ctrl+J: {help_enter}"
    );
    let help_ctrl = keybinding_help(SubmitKeymap::CtrlJ);
    assert_ne!(help_enter, help_ctrl);
    let hints_ctrl = footer_hints(SubmitKeymap::CtrlJ);
    assert!(!hints_ctrl.is_empty());
    // advertised submit key differs per keymap
    let submit_hint = |hs: &[KeyHint]| {
        hs.iter()
            .find(|h| h.action == "submit")
            .map(|h| h.keys.to_string())
            .unwrap()
    };
    assert_ne!(submit_hint(&hints_enter), submit_hint(&hints_ctrl));
}
