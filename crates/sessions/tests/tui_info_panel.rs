// UI-019 contract tests: right-side TUI info panel, bounded live metadata.
// Maps to obligations UI-019-T01..T05 in tasks/UI-019.md.
use opencode_rk_sessions::tui_info_panel::{render, summarize, InfoError, InfoSnapshot};

fn snapshot() -> InfoSnapshot {
    InfoSnapshot {
        cwd_label: "/home/user/project".to_owned(),
        workspace: "acme-web".to_owned(),
        session_label: "sess-01".to_owned(),
        provider_id: "openai".to_owned(),
        model: "gpt-5-mini".to_owned(),
        auth_mode: "api-key".to_owned(),
        ctx_window: 200_000,
        ctx_used: 12_500,
        tokens_in: 8_000,
        tokens_out: 4_500,
        cost_micros: Some(1_250),
        mcp_connected: true,
        mcp_enabled: 2,
        mcp_disabled: 1,
        active_tools: vec!["read".to_owned(), "write".to_owned(), "grep".to_owned()],
        queue_depth: 1,
        status_line: "streaming".to_owned(),
        updated_ms: 90_000,
        warnings: vec!["slow network".to_owned()],
    }
}

#[test]
fn ui019_t01_full_render() {
    let lines = render(&snapshot(), 120).unwrap();
    let joined = lines.join("\n");
    assert!(lines.iter().all(|l| l.len() <= 120));
    for needle in [
        "/home/user/project",
        "openai",
        "gpt-5-mini",
        "api-key",
        "12500",
        "187500",
        "8000",
        "4500",
        "enabled=2",
        "read",
        "queue=1",
        "90000",
        "slow network",
    ] {
        assert!(joined.contains(needle), "missing {needle}");
    }
}

#[test]
fn ui019_t02_narrow_terminal() {
    let lines = render(&snapshot(), 50).unwrap();
    assert!(lines.iter().all(|l| l.len() <= 50));
    let counts = summarize(&snapshot());
    assert_eq!(
        (counts.tools_shown, counts.warnings_shown, counts.truncated),
        (3, 1, false)
    );
    assert!(lines.join("\n").contains("openai"));
}

#[test]
fn ui019_t03_truncation_honesty() {
    let mut s = snapshot();
    s.active_tools = (0..30).map(|i| format!("tool-{i:02}")).collect();
    s.warnings = (0..20).map(|i| format!("warn-{i}")).collect();
    let counts = summarize(&s);
    assert_eq!(counts.tools_shown, 16);
    assert_eq!(counts.warnings_shown, 8);
    assert!(counts.truncated);
    let lines = render(&s, 120).unwrap();
    let joined = lines.join("\n");
    assert!(joined.contains("+14 more"));
    assert!(joined.contains("+12 more"));
    assert_eq!(s.ctx_window.saturating_sub(s.ctx_used), 187_500);
}

#[test]
fn ui019_t04_validation() {
    let mut s = snapshot();
    s.workspace = "   ".to_owned();
    assert_eq!(render(&s, 120), Err(InfoError::EmptyField));
    assert_eq!(render(&snapshot(), 10), Err(InfoError::TooNarrow));
    let mut big = snapshot();
    big.ctx_window = u64::MAX;
    big.ctx_used = u64::MAX;
    big.tokens_in = u64::MAX;
    let lines = render(&big, 120).unwrap();
    assert!(lines.iter().all(|l| l.len() <= 120));
}

#[test]
fn ui019_t05_redaction_and_determinism() {
    let mut s = snapshot();
    s.status_line = "ok".to_owned();
    let lines = render(&s, 120).unwrap();
    let joined = lines.join("\n");
    assert!(joined.contains("Tokens"));
    // A label carrying an sk-style secret is scrubbed.
    let mut evil = snapshot();
    evil.session_label = "sess-01 sk-fixture-secret-xyz".to_owned();
    let scrubbed = render(&evil, 120).unwrap().join("\n");
    assert!(!scrubbed.contains("sk-fixture-secret-xyz"));
    assert!(!scrubbed.contains("sk-"));
    let a = render(&snapshot(), 120).unwrap();
    let b = render(&snapshot(), 120).unwrap();
    assert_eq!(a, b);
}
