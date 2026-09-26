# BRIDGE-PAR-223 scratchpad

claim: BRIDGE-PAR-223 via ses_par223, scratchpad worklog/BRIDGE-PAR-223.md
source: crates/opentui-bridge/src/kv_ctx.rs:18 KvCtx {set,get,del,keys}, caps 128/64/512
target: crates/opentui-bridge/src/solid_store_full.rs only (lib.rs, Cargo.toml, kv_ctx.rs untouched)
tests: 5 unit tests in-file (new_zero, set_bump, overwrite_bump, reject_no_bump, get_roundtrip)
decisions: wrapping_add for version (no overflow panic); version bump only on kv.set true; std-only, forbid(unsafe_code)
unknowns: none; module unwired (lib.rs edit forbidden by scope)
