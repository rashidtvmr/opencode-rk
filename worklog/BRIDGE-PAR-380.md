# BRIDGE-PAR-380 (unclaimed, file-only per spawn orders)

Claim: none. Did NOT touch claims.json (orchestrator owns). File-only lane.
Source: packages/tui/src/context/helper.tsx:1-26 createSimpleContext show-gate (ready undefined/true renders, use throws outside provider).
Existing: crates/opentui-bridge/src/helper_ctx.rs (HelperCtx visible/topic, MAX_TOPIC_LEN 64) differs; new file is named readiness gate per deliverable.
Target: crates/opentui-bridge/src/ctx_helper_full.rs, no lib.rs/Cargo.toml edits.
Tests: 3 inline (default_empty_not_ready, set_name_caps_at_64, set_ready_toggles). Not run (no cargo per scope); rustfmt --check PASS, 60 lines (<70).
Decisions: fields private with accessors per spec; char-based truncation; forbid(unsafe_code); std-only.
Unknowns: wiring into lib.rs left to orchestrator (out of scope).
