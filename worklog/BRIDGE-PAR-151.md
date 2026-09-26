# BRIDGE-PAR-151 scratchpad

Claim: BRIDGE-PAR-151 via ses_par151.
Evidence: TS event.ts:12-30 subscribe/on closure; core_events.rs route_event RenderEvent->EventKind; sibling slot_registry.rs mount/unmount/owners_of pattern.
Target: ONE file crates/opentui-bridge/src/event_ctx.rs, EventCtx + sub/unsub/emit, caps 64/64/64, forbid(unsafe_code), <150 lines.
Tests: 5 in-file (sub/emit order, unsub, missing empty, dup+rejects, cap). Written, unrun per scope (fmt only).
Decision: Vec<(String,String)> insertion-order store; emit clones owner ids filtered by ev; sync-filter left to SDK layer per TS truth.
Unknowns: none. No lib.rs/Cargo.toml touch, no cargo, no commit.
