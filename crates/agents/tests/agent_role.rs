//! RED lane AGENT-MODE-ROLE: role prompting + tool advertisement derived
//! from loaded agent definition files (`.agents/agents/*`).
//!
//! Frozen contract (TDD):
//! - T01 `AgentDef::role_prompt` for a `build`/absent mode returns the body
//!   text verbatim (mode directive omitted).
//! - T02 `plan` mode prefixes a read-only directive and keeps the body.
//! - T03 `AgentFileSnapshot::find` resolves by name, None otherwise.
//! - T04 `advertised_tools` intersects the enabled set with the file's tool
//!   allowlist; an empty allowlist with a non-plan mode passes everything.
//! - T05 `plan` mode with an empty allowlist advertises NO tools (fail-closed)
//!   and with an allowlist advertises only listed enabled tools.
#![forbid(unsafe_code)]

#[path = "../src/agent_files.rs"]
mod agent_files;

use agent_files::{AgentDef, AgentFileSnapshot};

fn def(name: &str, mode: Option<&str>, tools: &[&str], body: &str) -> AgentDef {
    AgentDef {
        name: name.to_string(),
        model: None,
        body: body.as_bytes().to_vec(),
        temperature: None,
        tools: tools.iter().map(|t| t.to_string()).collect(),
        mode: mode.map(|m| m.to_string()),
        path: std::path::PathBuf::from(format!("/agents/{name}.md")),
    }
}

fn enabled(tools: &[&str]) -> Vec<String> {
    tools.iter().map(|t| t.to_string()).collect()
}

#[test]
fn t01_role_prompt_build_mode_is_body() {
    let d = def("coder", Some("build"), &[], "You are a coder.");
    assert_eq!(d.role_prompt(), "You are a coder.");
    let d2 = def("plain", None, &[], "Body only.");
    assert_eq!(d2.role_prompt(), "Body only.");
}

#[test]
fn t02_role_prompt_plan_mode_adds_directive() {
    let d = def("planner", Some("plan"), &[], "Draft a plan.");
    let prompt = d.role_prompt();
    assert!(
        prompt.contains("plan mode"),
        "plan directive missing: {prompt:?}"
    );
    assert!(
        prompt.contains("Do not modify"),
        "read-only guard missing: {prompt:?}"
    );
    assert!(prompt.ends_with("Draft a plan."), "body must be kept");
}

#[test]
fn t03_snapshot_find_by_name() {
    let snap = AgentFileSnapshot {
        defs: vec![def("alpha", None, &[], "a"), def("beta", None, &[], "b")],
    };
    assert_eq!(snap.find("beta").map(|d| d.name.as_str()), Some("beta"));
    assert!(snap.find("gamma").is_none());
}

#[test]
fn t04_advertised_tools_intersects_and_passthrough() {
    // explicit allowlist: only listed + enabled survive, order follows allowlist
    let d = def("scoped", Some("build"), &["read", "edit"], "x");
    assert_eq!(
        d.advertised_tools(&enabled(&["read", "edit", "bash"])),
        enabled(&["read", "edit"])
    );
    // empty allowlist, non-plan: pass everything enabled
    let d2 = def("wide", Some("build"), &[], "x");
    assert_eq!(
        d2.advertised_tools(&enabled(&["read", "bash"])),
        enabled(&["read", "bash"])
    );
    // tools listed but not enabled are dropped
    let d3 = def("narrow", None, &["edit"], "x");
    assert_eq!(d3.advertised_tools(&enabled(&["read"])), Vec::<String>::new());
}

#[test]
fn t05_plan_mode_fail_closed_tools() {
    let d = def("plan-empty", Some("plan"), &[], "x");
    assert!(
        d.advertised_tools(&enabled(&["read", "bash"])).is_empty(),
        "plan mode with no allowlist must advertise nothing"
    );
    let d2 = def("plan-listed", Some("plan"), &["read"], "x");
    assert_eq!(
        d2.advertised_tools(&enabled(&["read", "bash"])),
        enabled(&["read"])
    );
}
