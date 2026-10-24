//! External MCP effects require a genuine broker-minted human capability.
use opencode_rk_security::{Decision, OperationIntent, PermissionBroker, SecurityPolicy};

#[test]
fn external_mcp_tools_require_human_authority_and_approval_binds_actual_call_id() {
    let root = tempfile::tempdir().unwrap();
    let broker = PermissionBroker::new(SecurityPolicy::lean_default(root.path()));
    let intent = OperationIntent::Tool { name: "mcp_01234567_probe".into(), description: "exact fixture arguments".into() };
    assert!(matches!(broker.authorize(&intent), Decision::RequireHuman { .. }),
        "MCP must be human-gated by the broker, not just by a pending UI prompt");
    let proof = broker.approve_human(&intent, "fixture-session".into(), 42, "call-real-id".into()).expect("a live human reply must be able to mint MCP authority");
    assert!(matches!(broker.authorize_with_human_approval(&intent, proof, "fixture-session", 42, "different-call"), Decision::RequireHuman { .. }));
    let proof = broker.approve_human(&intent, "fixture-session".into(), 42, "call-real-id".into()).unwrap();
    assert!(matches!(broker.authorize_with_human_approval(&intent, proof, "fixture-session", 42, "call-real-id"), Decision::Allow));
}
