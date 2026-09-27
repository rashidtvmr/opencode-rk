//! RED lane AGENT-MODE-ROLE (server half): resolve a turn's agent name to a
//! role prompt + advertised-tool restriction from `<workspace>/.agents/agents`.
//!
//! Frozen contract:
//! - T01 no agent name → no role (turn proceeds unchanged).
//! - T02 agent name with no definitions directory → ignored fail-open (None).
//! - T03 agent name matching a definition file → role prompt + tool
//!   intersection from that file.
//! - T04 definitions directory exists but the name is unknown → Err.
//! - T05 plan-mode agent restricts tools fail-closed (empty intersection).
#![forbid(unsafe_code)]

#[path = "../src/agent_roles.rs"]
mod agent_roles;

use std::fs;
use std::path::PathBuf;

use agent_roles::{resolve_agent_role, RoleError};

fn workspace(label: &str) -> PathBuf {
    let dir = PathBuf::from(format!(
        "/tmp/agent_roles_test_{}_{}",
        std::process::id(),
        label
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join(".agents/agents")).expect("mkdir");
    dir
}

fn write_agent(dir: &PathBuf, content: &str) {
    fs::write(dir.join(".agents/agents/coder.md"), content).unwrap();
}

const CODER_MD: &str = "---\nname: coder\ntools: [read, bash]\nmode: build\n---\nYou are the coder subagent.";

#[test]
fn t01_no_name_no_role() {
    let ws = workspace("t01");
    write_agent(&ws, CODER_MD);
    assert!(resolve_agent_role(&ws, None, &["read".into()]).unwrap().is_none());
    fs::remove_dir_all(&ws).unwrap();
}

#[test]
fn t02_missing_dir_is_fail_open() {
    let ws = workspace("t02");
    fs::remove_dir_all(ws.join(".agents")).unwrap();
    let r = resolve_agent_role(&ws, Some("coder"), &["read".into()]).unwrap();
    assert!(r.is_none(), "no definitions dir must not error or role-filter");
    fs::remove_dir_all(&ws).unwrap();
}

#[test]
fn t03_matching_agent_yields_role_and_tools() {
    let ws = workspace("t03");
    write_agent(&ws, CODER_MD);
    let r = resolve_agent_role(&ws, Some("coder"), &["read", "bash", "edit"])
        .unwrap()
        .expect("coder must resolve");
    assert_eq!(r.role_prompt, "You are the coder subagent.");
    assert_eq!(r.tools, vec!["read".to_string(), "bash".to_string()]);
    fs::remove_dir_all(&ws).unwrap();
}

#[test]
fn t04_unknown_name_with_defs_errors() {
    let ws = workspace("t04");
    write_agent(&ws, CODER_MD);
    match resolve_agent_role(&ws, Some("ghost"), &["read".into()]) {
        Err(RoleError::UnknownAgent(name)) => assert_eq!(name, "ghost"),
        other => panic!("expected UnknownAgent, got {other:?}"),
    }
    fs::remove_dir_all(&ws).unwrap();
}

#[test]
fn t05_plan_agent_fail_closed_tools() {
    let ws = workspace("t05");
    fs::write(
        ws.join(".agents/agents/planner.md"),
        "---\nname: planner\nmode: plan\n---\nDraft a plan first.",
    )
    .unwrap();
    let r = resolve_agent_role(&ws, Some("planner"), &["read", "bash"])
        .unwrap()
        .expect("planner must resolve");
    assert!(r.role_prompt.contains("plan mode"));
    assert!(r.role_prompt.ends_with("Draft a plan first."));
    assert!(r.tools.is_empty(), "plan mode advertises no tools");
    fs::remove_dir_all(&ws).unwrap();
}
