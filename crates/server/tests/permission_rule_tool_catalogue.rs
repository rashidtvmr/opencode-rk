//! Boundary pin: the persistent rule catalogue accepts exactly the 10 editor tools,
//! rejects everything else without mutation, and keeps session denies session-local.
//! Executor/role-tool behavior is covered elsewhere; this file mutates only the rules file.
#![forbid(unsafe_code)]

use opencode_rk_server::permission_rules::{decide, list, upsert, Rule};
use std::path::PathBuf;

const TOOLS: [&str; 10] = [
    "bash", "shell", "read", "write", "edit", "grep", "glob", "websearch", "webfetch", "task",
];
const SESSION_A: &str = "00000000-0000-4000-8000-00000000000a";
const SESSION_B: &str = "00000000-0000-4000-8000-00000000000b";

struct RulesEnv {
    previous: Option<std::ffi::OsString>,
}
impl RulesEnv {
    fn install(path: PathBuf) -> Self {
        let previous = std::env::var_os("OPENCODE_RK_PERMISSION_RULES");
        std::env::set_var("OPENCODE_RK_PERMISSION_RULES", path);
        Self { previous }
    }
}
impl Drop for RulesEnv {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var("OPENCODE_RK_PERMISSION_RULES", value),
            None => std::env::remove_var("OPENCODE_RK_PERMISSION_RULES"),
        }
    }
}

fn rule(tool: &str, decision: &str, scope: &str, session_id: Option<&str>) -> Rule {
    Rule {
        tool: tool.into(),
        decision: decision.into(),
        scope: scope.into(),
        session_id: session_id.map(Into::into),
        resource: None,
        context: None,
    }
}

#[test]
fn catalogue_boundary_is_exact_and_session_scoped() {
    let _dir = tempfile::TempDir::new().unwrap();
    let _env = RulesEnv::install(_dir.path().join("permission-rules.json"));

    for tool in TOOLS {
        upsert(rule(tool, "deny", "always", None)).unwrap();
        let rules = list(None).unwrap();
        assert!(rules.iter().any(|r| r.tool == tool && r.decision == "deny" && r.scope == "always"));
        assert_eq!(decide(tool, SESSION_A, "", ""), Ok(Some(false)), "{tool}");
    }
    let stored = list(None).unwrap();
    assert_eq!(stored.len(), TOOLS.len());
    assert!(TOOLS.iter().all(|tool| stored.iter().any(|r| r.tool == *tool)));

    // Unknown and arbitrary tools are rejected and leave the catalogue untouched.
    let before = list(None).unwrap();
    for tool in ["*", "exec", "BASH", ""] {
        assert!(upsert(rule(tool, "deny", "always", None)).is_err(), "{tool:?}");
    }
    assert_eq!(list(None).unwrap(), before);
    assert_eq!(decide("*", SESSION_A, "", ""), Ok(None));

    // `ask` resets only the exact tool: shell disappears, bash stays denied.
    upsert(rule("shell", "ask", "always", None)).unwrap();
    let after_reset = list(None).unwrap();
    assert!(after_reset.iter().all(|r| r.tool != "shell"));
    assert_eq!(after_reset.len(), TOOLS.len() - 1);
    assert_eq!(decide("shell", SESSION_A, "", ""), Ok(None));
    assert_eq!(decide("bash", SESSION_A, "", ""), Ok(Some(false)));

    // Global allow, then a session deny for A only: B keeps the allow.
    upsert(rule("read", "ask", "always", None)).unwrap();
    upsert(rule("read", "allow", "always", None)).unwrap();
    assert_eq!(decide("read", SESSION_B, "", ""), Ok(Some(true)));
    upsert(rule("read", "deny", "session", Some(SESSION_A))).unwrap();
    assert_eq!(decide("read", SESSION_A, "", ""), Ok(Some(false)));
    assert_eq!(decide("read", SESSION_B, "", ""), Ok(Some(true)));
    let durable = list(None).unwrap();
    assert_eq!(durable.len(), TOOLS.len() - 1); // shell removed, read now allow
    assert!(durable.iter().any(|r| r.tool == "read" && r.decision == "allow"));
}
