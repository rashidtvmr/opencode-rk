//! Qualified MCP preferences remain exact and cannot authorize another call.
use opencode_rk_contracts::SessionId;
use opencode_rk_security::{OperationIntent, PermissionBroker, SecurityPolicy};
use opencode_rk_server::{permission_rules::{self, Rule}, run_owner::{PermissionRequest, PermissionSource, TurnExecutionPermit}};

struct Environment(Option<std::ffi::OsString>);
impl Drop for Environment {
    fn drop(&mut self) {
        match &self.0 {
            Some(value) => std::env::set_var("OPENCODE_RK_PERMISSION_RULES", value),
            None => std::env::remove_var("OPENCODE_RK_PERMISSION_RULES"),
        }
    }
}

#[tokio::test]
async fn saved_mcp_grants_bind_exact_tool_arguments_context_and_session() {
    let directory = tempfile::tempdir().unwrap();
    let _environment = Environment(std::env::var_os("OPENCODE_RK_PERMISSION_RULES"));
    let path = directory.path().join("rules.json");
    std::env::set_var("OPENCODE_RK_PERMISSION_RULES", &path);
    let session = SessionId::new();
    let other_session = SessionId::new();
    let permit = TurnExecutionPermit::acquire(session).unwrap();
    let owner = permit.owner();
    let broker = PermissionBroker::new(SecurityPolicy::lean_default(directory.path()));
    let tool = "mcp_01234567_exact_probe";
    let intent = OperationIntent::Tool { name: tool.into(), description: r#"{"value":"exact"}"#.into() };
    let request = |id: &str, call: &str| PermissionRequest {
        id: id.into(), session_id: session.to_string(), action: tool.into(),
        resources: vec![serde_json::to_string(&intent).unwrap()],
        source: PermissionSource { kind: "tool", message_id: "fixture-message".into(), call_id: call.into() },
    };
    let receiver = owner.install_pending(request("first", "call-one"), intent.clone(), &broker).unwrap();
    assert!(owner.reply_scoped("first", true, "session").expect("qualified MCP session grants must be supported"));
    assert!(receiver.await.unwrap().capability.is_some());
    let resource = permission_rules::fingerprint(&serde_json::to_string(&intent).unwrap());
    let context = permission_rules::fingerprint(&format!("{:?}", broker.policy_context()));
    assert_eq!(permission_rules::decide(tool, &session.to_string(), &resource, &context).unwrap(), Some(true));
    assert_eq!(permission_rules::decide(tool, &other_session.to_string(), &resource, &context).unwrap(), None);
    let different = permission_rules::fingerprint("different arguments");
    assert_eq!(permission_rules::decide(tool, &session.to_string(), &different, &context).unwrap(), None);
    assert_eq!(permission_rules::decide(tool, &session.to_string(), &resource, &different).unwrap(), None);

    let other = "mcp_89abcdef_other_probe";
    permission_rules::upsert(Rule { tool: other.into(), decision: "deny".into(), scope: "always".into(), session_id: None, resource: Some(resource.clone()), context: None }).unwrap();
    assert_eq!(permission_rules::decide(other, &other_session.to_string(), &resource, &context).unwrap(), Some(false));
    assert!(path.is_file());
    assert_eq!(permission_rules::list(None).unwrap().len(), 1, "persistent rules must reload from disk");
    for name in ["mcp_*", "mcp_", "mcp_unsafe/name", "mcp_unsafe\nname"] {
        assert!(permission_rules::upsert(Rule { tool:name.into(), decision:"allow".into(), scope:"always".into(), session_id:None, resource:None, context:None }).is_err(), "invalid MCP tool name was accepted: {name:?}");
    }
}
