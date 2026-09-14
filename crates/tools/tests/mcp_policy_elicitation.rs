use opencode_rk_tools::mcp::{
    McpClient, McpConfig, McpError, McpPolicy, McpPolicyDecision, McpTool,
};
use serde_json::json;
use tokio::sync::oneshot;

const TOOL_NAME: &str = "fixture_tool";

fn client_with(policy: McpPolicy, timeout_secs: u64) -> McpClient {
    let config = McpConfig::new("fixture-mcp").with_timeout(timeout_secs);
    let mut client = McpClient::new(config).with_policy(policy);
    client.register_tool(McpTool::new(
        TOOL_NAME,
        "fixture MCP tool",
        json!({"type": "object"}),
    ));
    client
}

fn policy(decision: McpPolicyDecision) -> McpPolicy {
    let mut policy = McpPolicy::default_allow();
    policy.set_rule(TOOL_NAME, decision);
    policy
}

#[tokio::test]
async fn tool_015_t01_default_policy_preserves_existing_allow_behavior() {
    let mut client =
        McpClient::new(McpConfig::new("fixture-mcp")).with_policy(McpPolicy::default_allow());
    client.register_tool(McpTool::new(
        TOOL_NAME,
        "fixture MCP tool",
        json!({"type": "object"}),
    ));

    let result = client
        .call_tool(TOOL_NAME, json!({"input": "default"}))
        .await
        .expect("default MCP policy should preserve existing allow behavior");

    assert!(result.success);
    assert!(result.output.contains(TOOL_NAME));
}

#[tokio::test]
async fn tool_015_t02_explicit_deny_blocks_before_tool_execution() {
    let mut client = client_with(policy(McpPolicyDecision::Deny), 1);

    let error = client
        .call_tool(TOOL_NAME, json!({"input": "blocked"}))
        .await
        .expect_err("explicit MCP deny must block the tool call");

    assert!(matches!(
        error,
        McpError::PolicyDenied(tool) if tool == TOOL_NAME
    ));
}

#[tokio::test]
async fn tool_015_t03_explicit_allow_executes_the_tool() {
    let mut client = client_with(policy(McpPolicyDecision::Allow), 1);

    let result = client
        .call_tool(TOOL_NAME, json!({"input": "allowed"}))
        .await
        .expect("explicit MCP allow should execute the tool");

    assert!(result.success);
    assert!(result.output.contains("allowed"));
}

#[tokio::test]
async fn tool_015_t04_elicitation_approval_executes_the_tool() {
    let mut client = client_with(policy(McpPolicyDecision::Elicit), 1);
    let required = client
        .call_tool(TOOL_NAME, json!({"input": "requires-approval"}))
        .await
        .expect_err("Elicit policy must require caller-owned approval");
    assert!(matches!(
        required,
        McpError::ElicitationRequired(tool) if tool == TOOL_NAME
    ));

    let (approve, response) = oneshot::channel();
    approve
        .send(true)
        .expect("receiver should remain owned by the pending call");

    let result = client
        .call_tool_with_elicitation(TOOL_NAME, json!({"input": "approved"}), Some(response))
        .await
        .expect("approved elicitation should execute the tool");

    assert!(result.success);
    assert!(result.output.contains("approved"));
}

#[tokio::test]
async fn tool_015_t05_elicitation_denial_cancel_and_timeout_are_typed_errors() {
    let mut denied_client = client_with(policy(McpPolicyDecision::Elicit), 1);
    let (deny, denied_response) = oneshot::channel();
    deny.send(false)
        .expect("receiver should remain owned by the pending call");
    let denied = denied_client
        .call_tool_with_elicitation(TOOL_NAME, json!({}), Some(denied_response))
        .await
        .expect_err("negative elicitation response must deny execution");
    assert!(matches!(
        denied,
        McpError::ElicitationDenied(tool) if tool == TOOL_NAME
    ));

    let mut cancelled_client = client_with(policy(McpPolicyDecision::Elicit), 1);
    let (cancel, cancelled_response) = oneshot::channel::<bool>();
    drop(cancel);
    let cancelled = cancelled_client
        .call_tool_with_elicitation(TOOL_NAME, json!({}), Some(cancelled_response))
        .await
        .expect_err("dropped elicitation owner must cancel the call");
    assert!(matches!(
        cancelled,
        McpError::ElicitationCancelled(tool) if tool == TOOL_NAME
    ));

    let mut timed_out_client = client_with(policy(McpPolicyDecision::Elicit), 0);
    let (_pending_owner, timed_out_response) = oneshot::channel::<bool>();
    let timed_out = timed_out_client
        .call_tool_with_elicitation(TOOL_NAME, json!({}), Some(timed_out_response))
        .await
        .expect_err("McpConfig timeout must bound elicitation wait");
    assert!(matches!(
        timed_out,
        McpError::ElicitationTimeout {
            tool,
            timeout_secs: 0
        } if tool == TOOL_NAME
    ));
}
