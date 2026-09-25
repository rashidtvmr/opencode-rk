# BRIDGE-PAR-149 scratchpad

Claim: BRIDGE-PAR-149 via ses_par149, scratchpad worklog/BRIDGE-PAR-149.md.
Source evidence:
- TS truth: /home/rashid/projects/opencode/packages/tui/src/context/kv.tsx:51-56 (flat get/set, kv.json persist).
- Sibling (read-only): crates/opentui-bridge/src/context_kv.rs:30-89 (KvStore Result API, 256/128/4096).
Observed: sibling covers richer API; lane needs small bool fail-closed KvCtx 64/512/128.
Target boundary: ONE new file crates/opentui-bridge/src/kv_ctx.rs. No lib.rs/Cargo.toml/context_kv.rs/kv_toggles.rs edits. No cargo/commit.
Tests: 6 in-file #[cfg(test)] (set/get, overwrite, del-missing, caps, full-cap, keys-order).
Decisions: Vec pairs insertion-ordered; char-count caps; overwrite allowed when full; std-only; forbid(unsafe_code).
Remaining: none. rustfmt --check PASS. File 135 lines.
