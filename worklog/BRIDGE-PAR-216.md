# BRIDGE-PAR-216 scratchpad
- claim: BRIDGE-PAR-216 ses_par216 ok
- evidence: runtime_shared_full.rs RuntimeSharedFull{model,busy,turns,set_model,start_turn,end_turn,status}; runtime.ts:1-80 runInteractiveRuntime boot/lifecycle/transport/queue; run_runtime.rs RuntimeLifecycle/queue cap64/stdin
- target: crates/opentui-bridge/src/run_runtime_full.rs only; no lib.rs/Cargo.toml edits
- impl: RuntimeFull{rt,turns}+new/rt/turns/set_model/begin->start_turn/end->end_turn+saturating/status capped256; forbid unsafe, std-only
- tests: 6 (idle,guard,bumps,saturates,parts,capped)
- verify: rustfmt --check only (no cargo per scope)
