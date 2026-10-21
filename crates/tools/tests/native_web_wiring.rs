use opencode_rk_tools::{executor::{ToolCall, ToolExecutor}, registry::ToolRegistry};
use serde_json::json;

#[test]
fn native_web_tools_are_discoverable_with_real_argument_schemas() {
    let registry = ToolRegistry::new();
    for (name, required) in [("websearch", "query"), ("webfetch", "url")] {
        let tool = registry.get(name).expect("native web tool must be registered");
        assert!(tool.input_schema["required"].as_array().unwrap().contains(&json!(required)));
        assert!(tool.enabled);
    }
}

#[test]
fn native_web_intents_validate_before_network_and_bind_full_request() {
    let executor = ToolExecutor::new();
    let intent = executor.intent(&ToolCall::new("search-1", "websearch", json!({"query":"Rust release notes"})));
    assert!(intent.is_ok(), "{intent:?}");
    for url in ["http://127.0.0.1", "http://2130706433", "http://[::1]", "http://169.254.169.254"] {
        assert!(executor.intent(&ToolCall::new("fetch-1", "webfetch", json!({"url":url}))).is_err());
    }
}

#[tokio::test]
async fn unapproved_network_tools_never_start_a_request() {
    use opencode_rk_security::{PermissionBroker, SecurityPolicy};
    let dir = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new().with_broker(PermissionBroker::new(SecurityPolicy::lean_default(dir.path().to_owned())));
    for (name, input) in [("websearch",json!({"query":"Rust release notes"})), ("webfetch",json!({"url":"https://example.com"}))] {
        let result = executor.execute(ToolCall::new("unapproved",name,input)).await;
        assert!(!result.success);
        assert!(result.error.unwrap().contains("human approval"));
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let result = executor.execute(ToolCall::new("private", "webfetch",json!({"url":format!("http://{}",listener.local_addr().unwrap())}))).await;
    assert!(!result.success);
    assert!(tokio::time::timeout(std::time::Duration::from_millis(50),listener.accept()).await.is_err(), "refused tool must have no network side effect");
}
