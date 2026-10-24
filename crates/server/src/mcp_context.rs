//! Session-scoped MCP tool projection and guarded execution.
#![forbid(unsafe_code)]
use crate::{mcp_http::McpStatusRegistry, run_owner::{ApprovedPermission, PendingPermissionGuard, PermissionReply, PermissionRequest, PermissionSource, RunOwner}};
use opencode_rk_contracts::{MessageId, SessionId};
use opencode_rk_security::{Decision, OperationIntent, PermissionBroker};
use serde_json::{json, Value};
use std::sync::Arc;

pub const MAX_TOOL_NAME_BYTES: usize = 64;
pub const MAX_RESULT_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug)]
pub struct McpToolDefinition { pub name: String, pub description: String, pub input_schema: Value, pub server: String, pub tool: String }

pub fn mcp_advertised() -> bool {
    match std::env::var("OPENCODE_RK_TURN_TOOLS") {
        Ok(value) => value.split(',').any(|v| v.trim().eq_ignore_ascii_case("mcp")),
        Err(std::env::VarError::NotPresent) => true,
        Err(_) => false,
    }
}

fn stable_name(server: &str, tool: &str) -> String {
    let digest = blake3::hash(format!("{server}\0{tool}").as_bytes()).to_hex().to_string();
    let sanitized: String = tool.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    let keep = MAX_TOOL_NAME_BYTES.saturating_sub(13 + 1 + digest.len().min(8));
    format!("mcp_{}_{}", &digest[..8], sanitized.chars().take(keep).collect::<String>())
}

pub async fn effective_tools(registry: &McpStatusRegistry, session: SessionId, sessions: &opencode_rk_sessions::SessionService) -> Result<Vec<McpToolDefinition>, String> {
    if !mcp_advertised() { return Ok(Vec::new()); }
    let excluded = sessions.mcp_exclusions(session).await.map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for server in registry.connected_names().await.into_iter().filter(|s| !excluded.iter().any(|x| x == s)) {
        for tool in registry.effective_tools(&server).await {
            out.push(McpToolDefinition { name: stable_name(&server, &tool.name), description: format!("MCP {server}/{0}: {1}", tool.name, tool.description), input_schema: tool.input_schema, server: server.clone(), tool: tool.name });
        }
    }
    Ok(out)
}

pub async fn execute(registry: &McpStatusRegistry, sessions: &opencode_rk_sessions::SessionService, session: SessionId, owner: &Arc<RunOwner>, broker: &PermissionBroker, message_id: MessageId, call_id: &str, call_name: &str, arguments: Value) -> Result<(String, bool), String> {
    let defs = effective_tools(registry, session, sessions).await?;
    let Some(def) = defs.into_iter().find(|d| d.name == call_name) else { return Err("unknown or disabled MCP tool".into()); };
    let description = serde_json::to_string(&json!({"server":def.server,"tool":def.tool,"arguments":arguments})).map_err(|_| "invalid MCP arguments")?;
    if description.len() > MAX_RESULT_BYTES { return Err("MCP arguments too large".into()); }
    let intent = OperationIntent::Tool { name: def.name.clone(), description };
    if crate::permission_rules::denied(&def.name, &session.to_string(), &intent, broker) {
        return Ok(("MCP permission denied by saved rule".into(), false));
    }
    let request = PermissionRequest { id: format!("per_{}", opencode_rk_contracts::ApprovalId::new()), session_id: session.to_string(), action: def.name.clone(), resources: vec![intent_resource(&intent)], source: PermissionSource { kind: "tool", message_id: message_id.to_string(), call_id: call_id.to_owned() } };
    let request_id = request.id.clone();
    let receiver = owner.install_pending(request, intent.clone(), broker).map_err(|_| "permission request unavailable".to_owned())?;
    let _pending = PendingPermissionGuard::new(Arc::clone(owner), request_id);
    let mut cancelled = owner.cancellation_receiver();
    let reply = tokio::select! { biased; _ = cancelled.changed() => ApprovedPermission { reply: PermissionReply::Cancelled, capability: None }, reply = receiver => reply.unwrap_or(ApprovedPermission { reply: PermissionReply::Cancelled, capability: None }) };
    let proof = match reply.reply { PermissionReply::Once | PermissionReply::Always => reply.capability.ok_or("permission proof missing")?, PermissionReply::Reject => return Ok(("MCP permission rejected".into(), false)), PermissionReply::Cancelled => return Ok(("MCP call cancelled".into(), false)) };
    if owner.is_cancelled() { return Ok(("MCP call cancelled".into(), false)); }
    if !matches!(broker.authorize_with_human_approval(&intent, proof, &session.to_string(), owner.run_id, call_id), Decision::Allow) { return Ok(("MCP permission proof rejected".into(), false)); }
    // Inclusion may change while human approval is pending. Re-resolve the
    // exact mapping at the effect boundary rather than executing the earlier
    // advertisement snapshot after it has been excluded for this session.
    let current = effective_tools(registry, session, sessions).await?;
    if !current.iter().any(|tool| tool.name == call_name && tool.server == def.server && tool.tool == def.tool) {
        return Ok(("MCP tool disabled for this session".into(), false));
    }
    if owner.is_cancelled() { return Ok(("MCP call cancelled".into(), false)); }
    if crate::permission_rules::denied(&def.name, &session.to_string(), &intent, broker) {
        return Ok(("MCP permission denied by saved rule".into(), false));
    }
    let result = registry.call_tool_checked(&def.server, &def.tool, arguments).await?;
    let bytes = serde_json::to_vec(&result).map_err(|_| "invalid MCP result")?;
    if bytes.len() > MAX_RESULT_BYTES { return Ok(("MCP result exceeded 64KiB".into(), false)); }
    let is_error = result.get("isError").and_then(Value::as_bool).unwrap_or(false);
    Ok((result.to_string(), !is_error))
}

fn intent_resource(intent: &OperationIntent) -> String { serde_json::to_string(intent).unwrap_or_else(|_| "invalid MCP intent".into()) }

#[cfg(test)]
mod permission_boundary_tests {
    use super::*;
    use axum::{extract::{Extension, Path}, Json};
    use crate::run_owner::TurnExecutionPermit;

    #[tokio::test]
    async fn excluding_a_server_while_approval_is_pending_prevents_the_external_call() {
        let root = tempfile::tempdir().unwrap();
        let script = root.path().join("pending-selection.sh");
        let marker = root.path().join("tool-was-called");
        std::fs::write(&script, r###"#!/bin/sh
set -eu
while IFS= read -r body; do
  id=$(printf '%s' "$body" | sed -n 's/.*"id"[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p')
  case "$body" in
    *'"method":"initialize"'*) out="{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{\"tools\":{}},\"serverInfo\":{\"name\":\"selection-fixture\",\"version\":\"1\"}}}";;
    *'"method":"tools/list"'*) out="{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"tools\":[{\"name\":\"probe\",\"description\":\"pending selection probe\",\"inputSchema\":{\"type\":\"object\"}}]}}";;
    *'"method":"tools/call"'*) printf 'called' > "$1"; out="{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"called\"}],\"isError\":false}}";;
    *) continue;;
  esac
  printf '%s\n' "$out"
done
"###).unwrap();
        let registry = McpStatusRegistry::new();
        crate::mcp_http::add(Extension(registry.clone()), Ok(Json(json!({
            "name":"pending-probe", "config":{"type":"local", "command":["/bin/sh",script,marker], "environment":{},"enabled":true}
        })))).await.unwrap();
        crate::mcp_http::connect(Extension(registry.clone()), Path("pending-probe".into())).await.unwrap();
        let sessions = opencode_rk_sessions::SessionService::new(Arc::new(
            opencode_rk_storage::Storage::open_in_memory(root.path().join("blobs")).unwrap()));
        let session = sessions.create("Pending MCP selection").await.unwrap();
        let defs = effective_tools(&registry, session.id, &sessions).await.unwrap();
        let name = defs.first().expect("fixture tools are enabled in this test").name.clone();
        let permit = TurnExecutionPermit::acquire(session.id).unwrap();
        let owner = permit.owner();
        let worker_owner = owner.clone();
        let worker_registry = registry.clone();
        let worker_sessions = sessions.clone();
        let broker = PermissionBroker::new(opencode_rk_security::SecurityPolicy::lean_default(root.path()));
        let worker = tokio::spawn(async move {
            execute(&worker_registry, &worker_sessions, session.id, &worker_owner, &broker,
                MessageId::new(), "pending-call-id", &name, json!({})).await
        });
        let pending = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if let Some(request) = owner.pending() { break request; }
                tokio::task::yield_now().await;
            }
        }).await.expect("real execution must request human approval");
        sessions.set_mcp_included(session.id, "pending-probe".into(), false).await.unwrap();
        assert!(owner.reply_once(&pending.id));
        let outcome = tokio::time::timeout(std::time::Duration::from_secs(5), worker).await.unwrap().unwrap();
        crate::mcp_http::disconnect(Extension(registry), Path("pending-probe".into())).await.unwrap();
        assert!(!marker.exists(), "a session-excluded MCP server must receive no tools/call after pending approval");
        assert!(outcome.is_err() || !outcome.unwrap().1);
    }
}
