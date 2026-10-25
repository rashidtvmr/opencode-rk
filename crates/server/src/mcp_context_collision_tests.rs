//! Regression: deterministic search for an MCP qualified-name (8-hex) collision
//! reproducing the security identity collision concern diagnosed in
//! docs/worklogs/context-controls-004/mcp-qualified-name-regression-handoff.md.
//!
//! No production fix here: this is the RED reproduction. The deterministic
//! search scans server IDs `fixture-server-{i}` for `i in 0..250000` with the
//! same ASCII tool `probe`, computing the LEGACY 8-hex identity only. It records
//! the first legacy-identity collision, then asserts the CURRENT `stable_name`
//! produces distinct names for the colliding pair -- failing on 8-hex while
//! bounding memory (~30 MB HashMap) and CPU (~1 s) per the handoff contract.
#![forbid(unsafe_code)]

use super::{MAX_TOOL_NAME_BYTES, stable_name};
use std::collections::HashMap;

/// Legacy 8-hex identity used historically by `stable_name` (mcp_context.rs:23-28).
/// Reconstructed here verbatim so the collision search is self-evidencing and
/// does not depend on the current `stable_name` implementation.
fn legacy_identity(server: &str, tool: &str) -> [u8; 8] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(server.as_bytes());
    hasher.update(b"\0");
    hasher.update(tool.as_bytes());
    let hex = hasher.finalize().to_hex();
    let mut out = [0u8; 8];
    let s = hex.as_str();
    for (i, b) in s.as_bytes().iter().take(8).enumerate() {
        out[i] = *b;
    }
    out
}

#[test]
fn legacy_8hex_collision_is_reproduced_and_current_name_distinguishes_pair() {
    // Deterministic scan of server IDs with identical ASCII tool `probe`.
    let tool = "probe".to_string();
    // The legacy digest has 2^32 possible values; this deterministic search
    // reaches its known collision before 250_000 candidates.
    // cap keeps the test bounded (~30 MB) and fast (~1 s) on commodity hardware.
    let limit = 250_000usize;

    let mut seen: HashMap<[u8; 8], String> = HashMap::with_capacity(1 << 14);
    let mut collision: Option<(String, String)> = None;

    for i in 0..limit {
        let server = format!("fixture-server-{i}");
        let legacy = legacy_identity(&server, &tool);
        if let Some(first) = seen.get(&legacy) {
            collision = Some((first.clone(), server));
            break;
        }
        seen.insert(legacy, server);
    }

    let (first_server, second_server) = collision.expect(
        "deterministic legacy 8-hex collision must surface within 250_000 server IDs \
         for tool 'probe'; if this fails, the blake3 truncation contract changed",
    );

    // Assert the CURRENT stable_name does NOT silently collapse the colliding
    // pair onto a single qualified name (the security failure mode).
    let first_name = stable_name(&first_server, &tool);
    let second_name = stable_name(&second_server, &tool);
    assert_ne!(
        first_name, second_name,
        "stable_name must not reuse the colliding legacy 8-hex alias for distinct (server, tool) pairs \
         (first_server={first_server}, second_server={second_server}, name={first_name})",
    );

    // The names must remain valid per the 64-byte ASCII invariant even for a
    // long non-ASCII tool whose sanitized form approaches the bound.
    let long_non_ascii: String =
        "tool".to_string() + &"_".repeat(MAX_TOOL_NAME_BYTES) + "éxördïnary";
    let name = stable_name(&first_server, &long_non_ascii);
    assert!(name.is_ascii(), "provider tool names must remain ASCII");
    assert!(
        name.len() <= MAX_TOOL_NAME_BYTES,
        "stable_name must cap at {MAX_TOOL_NAME_BYTES} bytes, got {}",
        name.len()
    );
    assert!(
        name.starts_with("mcp_"),
        "stable_name must preserve the mcp_ prefix"
    );
}
