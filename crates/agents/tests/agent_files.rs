#[path = "../src/agent_files.rs"]
mod agent_files;

use std::fs;
use std::path::PathBuf;

use agent_files::{load_agent_files, AgentDef, AgentFileError, AgentFileSnapshot};

fn tmp_dir(label: &str) -> PathBuf {
    let dir = PathBuf::from(format!(
        "/tmp/agent_files_test_{}_{}",
        std::process::id(),
        label
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create tmp dir");
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = fs::remove_dir_all(dir);
}

// ── T01: markdown agent file with frontmatter is discovered and parsed ──

#[test]
fn t01_markdown_frontmatter_parsed() {
    let ws = tmp_dir("t01");
    fs::write(
        ws.join("code_reviewer.md"),
        "---\nname: code_reviewer\nmodel: gpt-4\ntemperature: 0.3\nmode: build\ntools:\n  - read\n  - edit\n---\nYou are a code reviewer.",
    )
    .unwrap();
    let snap = load_agent_files(&ws).unwrap();
    assert_eq!(snap.defs.len(), 1);
    let d = &snap.defs[0];
    assert_eq!(d.name, "code_reviewer");
    assert_eq!(d.model.as_deref(), Some("gpt-4"));
    let temp = d.temperature.expect("temperature should be set");
    assert!((temp - 0.3).abs() < f64::EPSILON);
    assert_eq!(d.mode.as_deref(), Some("build"));
    assert_eq!(d.tools, vec!["read", "edit"]);
    assert_eq!(d.body, b"You are a code reviewer.");
    assert!(d.path.ends_with("code_reviewer.md"));
    cleanup(&ws);
}

// ── T02: json agent file is discovered and parsed ──

#[test]
fn t02_json_agent_parsed() {
    let ws = tmp_dir("t02");
    let agent = serde_json::json!({
        "name": "planner",
        "model": "claude-3",
        "temperature": 0.7,
        "mode": "plan",
        "tools": ["bash", "read"],
        "prompt": "You are a planning agent."
    });
    fs::write(ws.join("planner.json"), serde_json::to_string_pretty(&agent).unwrap()).unwrap();
    let snap = load_agent_files(&ws).unwrap();
    assert_eq!(snap.defs.len(), 1);
    let d = &snap.defs[0];
    assert_eq!(d.name, "planner");
    assert_eq!(d.model.as_deref(), Some("claude-3"));
    let temp = d.temperature.expect("temperature should be set");
    assert!((temp - 0.7).abs() < f64::EPSILON);
    assert_eq!(d.mode.as_deref(), Some("plan"));
    assert_eq!(d.tools, vec!["bash", "read"]);
    assert_eq!(d.body, b"You are a planning agent.");
    assert!(d.path.ends_with("planner.json"));
    cleanup(&ws);
}

// ── T03: deterministic ordering by path ──

#[test]
fn t03_deterministic_ordering() {
    let ws = tmp_dir("t03");
    for (name, body) in [
        ("zeta.md", "---\nname: zeta\n---\nbody"),
        ("alpha.json", r#"{"name":"alpha","prompt":"body"}"#),
        ("mu.md", "---\nname: mu\n---\nbody"),
    ] {
        fs::write(ws.join(name), body).unwrap();
    }
    let snap = load_agent_files(&ws).unwrap();
    let names: Vec<&str> = snap.defs.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "mu", "zeta"]);
    cleanup(&ws);
}

// ── T04: duplicate name produces error ──

#[test]
fn t04_duplicate_name_error() {
    let ws = tmp_dir("t04");
    fs::write(
        ws.join("alpha.md"),
        "---\nname: same_name\n---\nbody1",
    )
    .unwrap();
    fs::write(
        ws.join("beta.json"),
        r#"{"name":"same_name","prompt":"body2"}"#,
    )
    .unwrap();
    let result = load_agent_files(&ws);
    assert!(
        matches!(result, Err(AgentFileError::DuplicateName(ref n)) if n == "same_name"),
        "expected DuplicateName(\"same_name\"), got: {:?}",
        result,
    );
    cleanup(&ws);
}

// ── T05: symlink escape is rejected ──

#[test]
fn t05_symlink_escape_rejected() {
    let ws = tmp_dir("t05");
    let outside = PathBuf::from("/tmp/agent_files_escape_target");
    fs::write(&outside, r#"{"name":"escapee","prompt":"bad"}"#).unwrap();
    std::os::unix::fs::symlink(&outside, ws.join("sneaky.json")).unwrap();
    let result = load_agent_files(&ws);
    assert!(
        matches!(result, Err(AgentFileError::SymlinkEscape(_))),
        "symlink escape must be rejected, got: {:?}",
        result,
    );
    let _ = fs::remove_file(&outside);
    cleanup(&ws);
}

// ── T06: per-file size bound enforced ──

#[test]
fn t06_oversized_file_skipped() {
    let ws = tmp_dir("t06");
    let big = "x".repeat(agent_files::MAX_FILE_BYTES + 1);
    fs::write(ws.join("giant.md"), &big).unwrap();
    let snap = load_agent_files(&ws).unwrap();
    assert_eq!(snap.defs.len(), 0, "oversized file should be skipped");
    cleanup(&ws);
}

// ── T07: file count cap enforced ──

#[test]
fn t07_too_many_files_error() {
    let ws = tmp_dir("t07");
    for i in 0..=agent_files::MAX_FILES {
        fs::write(
            ws.join(format!("agent_{:04}.json", i)),
            r#"{"name":"x","prompt":"y"}"#,
        )
        .unwrap();
    }
    let result = load_agent_files(&ws);
    assert!(
        matches!(result, Err(AgentFileError::TooManyFiles(_))),
        "expected TooManyFiles, got: {:?}",
        result,
    );
    cleanup(&ws);
}

// ── T08: not-a-directory root ──

#[test]
fn t08_not_a_directory() {
    let file = PathBuf::from("/tmp/agent_files_notadir_file");
    fs::write(&file, "x").unwrap();
    let result = load_agent_files(&file);
    assert!(matches!(result, Err(AgentFileError::NotADirectory)));
    let _ = fs::remove_file(&file);
}

// ── T09: empty directory yields empty snapshot ──

#[test]
fn t09_empty_directory_empty_snapshot() {
    let ws = tmp_dir("t09");
    let snap = load_agent_files(&ws).unwrap();
    assert!(snap.defs.is_empty());
    cleanup(&ws);
}

// ── T10: markdown without frontmatter uses filename as name, full content as body ──

#[test]
fn t10_markdown_no_frontmatter() {
    let ws = tmp_dir("t10");
    fs::write(ws.join("bare_agent.md"), "Just a bare prompt.").unwrap();
    let snap = load_agent_files(&ws).unwrap();
    assert_eq!(snap.defs.len(), 1);
    let d = &snap.defs[0];
    assert_eq!(d.name, "bare_agent");
    assert_eq!(d.body, b"Just a bare prompt.");
    assert!(d.model.is_none());
    assert!(d.temperature.is_none());
    assert!(d.mode.is_none());
    assert!(d.tools.is_empty());
    cleanup(&ws);
}

// ── T11: json with missing optional fields still parses ──

#[test]
fn t11_json_minimal_fields() {
    let ws = tmp_dir("t11");
    fs::write(
        ws.join("minimal.json"),
        r#"{"name":"minimal","prompt":"do stuff"}"#,
    )
    .unwrap();
    let snap = load_agent_files(&ws).unwrap();
    assert_eq!(snap.defs.len(), 1);
    let d = &snap.defs[0];
    assert_eq!(d.name, "minimal");
    assert_eq!(d.body, b"do stuff");
    assert!(d.model.is_none());
    assert!(d.temperature.is_none());
    assert!(d.mode.is_none());
    assert!(d.tools.is_empty());
    cleanup(&ws);
}

// ── T12: unknown file extensions are ignored ──

#[test]
fn t12_unknown_extensions_ignored() {
    let ws = tmp_dir("t12");
    fs::write(ws.join("readme.txt"), "not an agent").unwrap();
    fs::write(ws.join("data.yaml"), "name: nope").unwrap();
    let snap = load_agent_files(&ws).unwrap();
    assert!(snap.defs.is_empty());
    cleanup(&ws);
}

// ── T13: json with "body" alias accepted as prompt ──

#[test]
fn t13_json_body_alias() {
    let ws = tmp_dir("t13");
    fs::write(
        ws.join("agent.json"),
        r#"{"name":"body_agent","body":"use body field"}"#,
    )
    .unwrap();
    let snap = load_agent_files(&ws).unwrap();
    assert_eq!(snap.defs.len(), 1);
    assert_eq!(snap.defs[0].body, b"use body field");
    cleanup(&ws);
}

// ── T14: total budget exceeded ──

#[test]
fn t14_total_budget_exceeded() {
    let ws = tmp_dir("t14");
    // Use content that fits within MAX_FILE_BYTES (account for frontmatter overhead).
    let content = "a".repeat(agent_files::MAX_FILE_BYTES - 64);
    let count = (agent_files::TOTAL_BUDGET / agent_files::MAX_FILE_BYTES) + 2;
    for i in 0..count {
        fs::write(
            ws.join(format!("agent_{:04}.md", i)),
            format!("---\nname: agent_{i}\n---\n{content}"),
        )
        .unwrap();
    }
    let result = load_agent_files(&ws);
    assert!(
        matches!(result, Err(AgentFileError::BudgetExceeded { .. })),
        "expected BudgetExceeded, got: {:?}",
        result,
    );
    cleanup(&ws);
}
