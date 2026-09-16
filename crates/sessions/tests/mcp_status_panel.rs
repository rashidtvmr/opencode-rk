// TOOL-020 contract tests: MCP status fragment with non-blocking publish.
// Maps to obligations TOOL-020-T01..T05 in tasks/TOOL-020.md.
use opencode_rk_sessions::mcp_status_panel::{
    publish, render, render_at, summarize, BoundedStatusBus, McpStatus, PublishOutcome,
    MAX_ERROR_LABEL_CHARS, MAX_STATUS_TOOLS,
};

fn status() -> McpStatus {
    McpStatus::new(
        true,
        2,
        1,
        ["read", "write", "grep"],
        Some("conn-refused".to_owned()),
        Some(120),
        90_000,
    )
}

#[test]
fn tool020_t01_panel_happy_path() {
    let lines = render(&status(), 120).unwrap();
    assert!(lines.len() >= 4);
    assert!(lines.iter().all(|l| l.len() <= 120));
    let joined = lines.join("\n");
    assert!(joined.contains("enabled=2"));
    assert!(joined.contains("disabled=1"));
    assert!(joined.contains("read"));
    assert!(joined.contains("conn-refused"));
    assert!(joined.contains("120ms"));
    assert!(joined.contains("90000"));
}

#[test]
fn tool020_t02_narrow_and_stale() {
    let lines = render(&status(), 50).unwrap();
    assert!(lines.iter().all(|l| l.len() <= 50));
    let fresh = render_at(&status(), 120, 95_000, 60_000).unwrap();
    assert!(!fresh.join("\n").contains("stale"));
    let stale = render_at(&status(), 120, 1_000_000, 60_000).unwrap();
    assert!(stale.join("\n").contains("stale"));
    assert_eq!(summarize(&status()), status().summarize());
    assert!(render(&status(), 10).is_err());
}

#[test]
fn tool020_t03_nonblocking_publish() {
    let bus = BoundedStatusBus::new(1);
    assert_eq!(publish(&bus, &status()), Ok(()));
    assert_eq!(publish(&bus, &status()), Err(PublishOutcome::Skipped));
    let events = bus.events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].enabled_count, 2);
    assert_eq!(events[0].disabled_count, 1);
    assert_eq!(bus.len(), 1);
}

#[test]
fn tool020_t04_truncation() {
    let tools: Vec<String> = (0..30).map(|i| format!("tool-{i:02}")).collect();
    let s = McpStatus::new(true, 1, 0, tools, None, None, 42);
    let counts = summarize(&s);
    assert_eq!(counts.tools_shown, MAX_STATUS_TOOLS as u64);
    assert!(counts.truncated);
    let lines = render(&s, 120).unwrap();
    assert!(lines.iter().all(|l| l.len() <= 120));
    assert!(lines.join("\n").contains("...[truncated]"));
    let long_err = "e".repeat(MAX_ERROR_LABEL_CHARS + 50);
    let s2 = McpStatus::new(true, 1, 0, Vec::<String>::new(), Some(long_err), None, 42);
    assert!(summarize(&s2).truncated);
    let rendered = render(&s2, 200).unwrap().join("\n");
    assert!(rendered.len() <= 5 * 200);
}

#[test]
fn tool020_t05_redaction_and_determinism() {
    let secret = "sk-fixture-secret-123";
    let s = McpStatus::new(
        true,
        1,
        0,
        ["read"],
        Some(format!("oops {secret}")),
        None,
        7,
    );
    let lines = render(&s, 120).unwrap();
    let joined = lines.join("\n");
    assert!(!joined.contains(secret));
    assert!(!joined.contains("sk-"));
    let bus = BoundedStatusBus::new(8);
    publish(&bus, &s).unwrap();
    let event_dbg = format!("{:?}", bus.events());
    assert!(!event_dbg.contains(secret));
    assert!(!event_dbg.contains("sk-"));
    let a = render(&status(), 120).unwrap();
    let b = render(&status(), 120).unwrap();
    assert_eq!(a, b);
}
