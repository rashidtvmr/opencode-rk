//! New session-state tools must support the same durable permission controls.
use opencode_rk_security::{OperationIntent, PermissionBroker, SecurityPolicy};
use opencode_rk_server::permission_rules::{self, Rule};

struct Environment(Option<std::ffi::OsString>);
impl Drop for Environment { fn drop(&mut self) {
    if let Some(value) = &self.0 { std::env::set_var("OPENCODE_RK_PERMISSION_RULES", value); }
    else { std::env::remove_var("OPENCODE_RK_PERMISSION_RULES"); }
} }

#[test]
fn todo_exact_resource_deny_is_durable_and_does_not_deny_another_update() {
    let directory = tempfile::tempdir().unwrap();
    let _environment = Environment(std::env::var_os("OPENCODE_RK_PERMISSION_RULES"));
    let path = directory.path().join("permission-rules.json");
    std::env::set_var("OPENCODE_RK_PERMISSION_RULES", &path);
    let session = opencode_rk_contracts::SessionId::new().to_string();
    let broker = PermissionBroker::new(SecurityPolicy::lean_default(directory.path()));
    let intent = OperationIntent::Tool { name: "todowrite".into(), description: r#"{"todos":[]}"#.into() };
    let resource = permission_rules::fingerprint(&serde_json::to_string(&intent).unwrap());
    assert!(!permission_rules::denied("todowrite", &session, &intent, &broker));
    permission_rules::upsert(Rule { tool: "todowrite".into(), decision: "deny".into(), scope: "always".into(), session_id: None, resource: Some(resource), context: None }).expect("todowrite must accept saved permission rules");
    assert!(path.is_file());
    // list/denied reload the actual private rules file on every call.
    assert_eq!(permission_rules::list(None).unwrap().len(), 1);
    assert!(permission_rules::denied("todowrite", &session, &intent, &broker));
    let different = OperationIntent::Tool { name: "todowrite".into(), description: r#"{"todos":[{"id":"different"}]}"#.into() };
    assert!(!permission_rules::denied("todowrite", &session, &different, &broker));
}
