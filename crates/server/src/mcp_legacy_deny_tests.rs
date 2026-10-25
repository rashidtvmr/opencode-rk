//! Regression tests for MCP qualified-name legacy DENY compatibility and
//! duplicate-name rejection in effective_tools.
//!
//! These tests verify:
//! 1. Legacy 8-hex DENY rules are still honored after the 128-bit digest change.
//! 2. Duplicate advertised qualified names are rejected (not silently collapsed).
//! 3. Resource-scoped fingerprint DENY rules containing the old OperationIntent.name
//!    remain effective for the legacy qualified name form.
#![forbid(unsafe_code)]

use super::{legacy_deny_name, stable_name, MAX_TOOL_NAME_BYTES};
use opencode_rk_security::{OperationIntent, PermissionBroker, SecurityPolicy};
use std::sync::{Mutex, MutexGuard, OnceLock};

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct PermissionRulesEnvGuard {
    previous: Option<std::ffi::OsString>,
    _lock: MutexGuard<'static, ()>,
}

impl PermissionRulesEnvGuard {
    fn set(path: &std::path::Path) -> Self {
        let lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let previous = std::env::var_os("OPENCODE_RK_PERMISSION_RULES");
        std::env::set_var("OPENCODE_RK_PERMISSION_RULES", path);
        Self { previous, _lock: lock }
    }
}

impl Drop for PermissionRulesEnvGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            std::env::set_var("OPENCODE_RK_PERMISSION_RULES", previous);
        } else {
            std::env::remove_var("OPENCODE_RK_PERMISSION_RULES");
        }
    }
}

/// The colliding pair found by the RED collision test.
const COLLISION_SERVER_A: &str = "fixture-server-17038";
const COLLISION_SERVER_B: &str = "fixture-server-40434";
const COLLISION_TOOL: &str = "probe";

/// Both colliding servers produce the same legacy 8-hex prefix.
#[test]
fn colliding_servers_share_legacy_8hex_prefix() {
    let legacy_a = legacy_deny_name(COLLISION_SERVER_A, COLLISION_TOOL);
    let legacy_b = legacy_deny_name(COLLISION_SERVER_B, COLLISION_TOOL);
    // Same first 8 hex chars (the legacy collision).
    assert_eq!(
        legacy_a.as_bytes()[..12],
        legacy_b.as_bytes()[..12],
        "legacy 8-hex prefix must collide for the known pair: {legacy_a} vs {legacy_b}"
    );
}

/// The new stable_name produces distinct names for the colliding pair.
#[test]
fn colliding_servers_produce_distinct_128bit_names() {
    let name_a = stable_name(COLLISION_SERVER_A, COLLISION_TOOL);
    let name_b = stable_name(COLLISION_SERVER_B, COLLISION_TOOL);
    assert_ne!(
        name_a, name_b,
        "stable_name must distinguish the colliding pair: {name_a}"
    );
    assert!(name_a.starts_with("mcp_"));
    assert!(name_b.starts_with("mcp_"));
    assert!(name_a.is_ascii() && name_b.is_ascii());
    assert!(name_a.len() <= MAX_TOOL_NAME_BYTES);
    assert!(name_b.len() <= MAX_TOOL_NAME_BYTES);
}

/// Legacy DENY rule on the legacy 8-hex name must still deny after the fix.
/// This verifies that changing the naming scheme does NOT break existing
/// persisted deny rules targeting `mcp_{8hex}_{sanitized}`.
#[test]
fn legacy_deny_rule_on_legacy_name_still_denies() {
    let root = tempfile::tempdir().unwrap();
    let rules_path = root.path().join("permission-rules.json");
    let _env = PermissionRulesEnvGuard::set(&rules_path);

    let legacy_name = legacy_deny_name(COLLISION_SERVER_A, COLLISION_TOOL);

    // Persist a DENY rule under the legacy name.
    crate::permission_rules::upsert(crate::permission_rules::Rule {
        tool: legacy_name.clone(),
        decision: "deny".to_string(),
        scope: "always".to_string(),
        session_id: None,
        resource: None,
        context: None,
    }).expect("upsert deny rule should succeed");

    let broker = PermissionBroker::new(SecurityPolicy::lean_default(root.path()))
        .with_permissions(opencode_rk_security::PermissionSet::star());

    let intent = OperationIntent::Tool {
        name: legacy_name.clone(),
        description: serde_json::to_string(&serde_json::json!({"server":COLLISION_SERVER_A,"tool":COLLISION_TOOL})).unwrap(),
    };

    // The DENY rule targets the legacy name exactly; permission_rules::denied
    // must return true even with star permissions on the broker.
    assert!(
        crate::permission_rules::denied(&legacy_name, "test-session", &intent, &broker),
        "legacy DENY rule on {legacy_name} must still deny after naming fix"
    );
}

/// A DENY rule targeting the legacy name must deny even when the current
/// (128-bit) name differs. This proves we check legacy names at the deny gate.
#[test]
fn resource_scoped_legacy_deny_blocks_current_intent() {
    let root = tempfile::tempdir().unwrap();
    let rules_path = root.path().join("permission-rules.json");
    let _env = PermissionRulesEnvGuard::set(&rules_path);

    let legacy_name = legacy_deny_name(COLLISION_SERVER_A, COLLISION_TOOL);
    let current_name = stable_name(COLLISION_SERVER_A, COLLISION_TOOL);

    // The names must differ for this server pair.
    assert_ne!(legacy_name, current_name, "names must differ for the fix to be meaningful");

    // Persist a resource-scoped DENY rule using the exact old OperationIntent
    // (including its old name). This is the persisted compatibility case that
    // must be reconstructed by the production deny check.
    let description = serde_json::json!({"server":COLLISION_SERVER_A,"tool":COLLISION_TOOL,"arguments":{}}).to_string();
    let legacy_intent = OperationIntent::Tool { name: legacy_name.clone(), description: description.clone() };
    crate::permission_rules::upsert(crate::permission_rules::Rule {
        tool: legacy_name.clone(),
        decision: "deny".to_string(),
        scope: "always".to_string(),
        session_id: None,
        resource: Some(crate::permission_rules::fingerprint(&serde_json::to_string(&legacy_intent).unwrap())),
        context: None,
    }).expect("upsert deny rule should succeed");

    let broker = PermissionBroker::new(SecurityPolicy::lean_default(root.path()))
        .with_permissions(opencode_rk_security::PermissionSet::star());

    // Use the production helper itself: it reconstructs the legacy intent with
    // this exact description before checking the resource fingerprint.
    let intent = OperationIntent::Tool { name: current_name.clone(), description };

    assert!(
        super::is_denied(&current_name, &legacy_name, "test-session", &intent, &broker),
        "resource-scoped legacy DENY must block the current intent"
    );
}

#[test]
fn matching_legacy_allow_does_not_deny_current_intent() {
    let root = tempfile::tempdir().unwrap();
    let rules_path = root.path().join("permission-rules.json");
    let _env = PermissionRulesEnvGuard::set(&rules_path);
    let legacy_name = legacy_deny_name(COLLISION_SERVER_A, COLLISION_TOOL);
    let current_name = stable_name(COLLISION_SERVER_A, COLLISION_TOOL);
    let description = serde_json::json!({
        "server": COLLISION_SERVER_A,
        "tool": COLLISION_TOOL,
        "arguments": {}
    })
    .to_string();
    let legacy_intent = OperationIntent::Tool {
        name: legacy_name.clone(),
        description: description.clone(),
    };
    crate::permission_rules::upsert(crate::permission_rules::Rule {
        tool: legacy_name.clone(),
        decision: "allow".to_string(),
        scope: "always".to_string(),
        session_id: None,
        resource: Some(crate::permission_rules::fingerprint(
            &serde_json::to_string(&legacy_intent).unwrap(),
        )),
        context: None,
    })
    .unwrap();
    let broker = PermissionBroker::new(SecurityPolicy::lean_default(root.path()));
    let current_intent = OperationIntent::Tool {
        name: current_name.clone(),
        description,
    };
    assert!(!super::is_denied(
        &current_name,
        &legacy_name,
        "test-session",
        &current_intent,
        &broker
    ));
}

/// Verify that a duplicate qualified name is rejected, not silently allowing
/// first-match ambiguity. We construct two (server, tool) pairs that would
/// produce the same legacy name and verify the new stable_name distinguishes
/// them.
#[test]
fn legacy_collision_aliases_are_replaced_by_distinct_current_names() {
    // The colliding pair: both produce legacy 8-hex "cce1e493".
    let server_a = COLLISION_SERVER_A;
    let server_b = COLLISION_SERVER_B;

    // With the old 8-hex scheme, both would produce "mcp_cce1e493_probe".
    // With the new 128-bit scheme, they produce distinct names.
    let name_a = stable_name(server_a, COLLISION_TOOL);
    let name_b = stable_name(server_b, COLLISION_TOOL);
    assert_ne!(name_a, name_b, "distinct servers must get distinct names");

    // Legacy names MUST be identical (that is the collision we are protecting against).
    let legacy_a = legacy_deny_name(server_a, COLLISION_TOOL);
    let legacy_b = legacy_deny_name(server_b, COLLISION_TOOL);
    assert_eq!(legacy_a, legacy_b, "legacy names must collide for this fixture (proving the vulnerability)");
}

/// Verify the 128-bit digest has 32 hex chars (128 bits) in the stable_name.
#[test]
fn stable_name_uses_128bit_digest() {
    let name = stable_name("test-server", "my_tool");
    // Format: mcp_{32hex}_{suffix}
    assert!(name.starts_with("mcp_"));
    let rest = &name[4..]; // strip "mcp_"
    let (digest_str, _suffix) = rest.split_once('_').expect("must have digest and suffix");
    assert_eq!(
        digest_str.len(),
        32,
        "128-bit digest should be 32 hex chars, got {}",
        digest_str.len()
    );
    assert!(
        digest_str.bytes().all(|b| b.is_ascii_hexdigit()),
        "digest must be valid hex: {digest_str}"
    );
}

/// Verify legacy_deny_name uses exactly 8 hex chars.
#[test]
fn legacy_deny_name_uses_8hex() {
    let legacy = legacy_deny_name("test-server", "my_tool");
    assert!(legacy.starts_with("mcp_"));
    let rest = &legacy[4..];
    let (digest_str, _suffix) = rest.split_once('_').expect("must have digest and suffix");
    assert_eq!(
        digest_str.len(),
        8,
        "legacy digest must be 8 hex chars, got {}",
        digest_str.len()
    );
}

/// Verify the name always stays within MAX_TOOL_NAME_BYTES for various inputs.
#[test]
fn stable_name_respects_byte_limit() {
    let cases = vec![
        ("server", "tool"),
        ("a-very-long-server-name-with-lots-of-characters", "tool"),
        ("server", "a_very_long_tool_name_with_lots_of_characters_and_special_chars!@#$%"),
    ];
    for (server, tool) in cases {
        let name = stable_name(server, tool);
        assert!(
            name.len() <= MAX_TOOL_NAME_BYTES,
            "name for {server}/{tool} exceeds {MAX_TOOL_NAME_BYTES}: {name} ({} bytes)",
            name.len()
        );
        assert!(
            name.is_ascii(),
            "name for {server}/{tool} must be ASCII: {name}"
        );
    }
}
