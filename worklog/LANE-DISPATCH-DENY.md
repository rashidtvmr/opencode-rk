# LANE-DISPATCH-DENY scratchpad
claim: LANE-DISPATCH-DENY / ses_f423cd0e1ffeTaoE4E1BlpxgdB
evidence:
- crates/tools/src/registry_dispatch.rs:64-78 AllowAll default-allow, Default impl returns true
- crates/tools/src/registry_dispatch.rs:132-135 RegistryDispatcher::new() defaults to AllowAll
- crates/tools/src/registry_dispatch.rs:218-227 dispatch_batch takes executor but `let _ = executor;` discards; :272-274 spawns `ToolExecutor::new()` per task (loses custom TimeoutConfig)
- crates/tools/src/executor.rs:79-94 ToolExecutor has with_timeout_config; execute() honors per-call + max cap; Clone absent
- broker map: crates/security/src/lib.rs:154 PermissionBroker exists; DispatchPolicy is local trait, no broker wiring yet
target: deny-by-default dispatch; batch honors injected executor config
tests: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 110 cargo test -p opencode-rk-tools --lib registry_dispatch` (no test edits)
decisions: TBD (DenyAll default vs explicit-policy-only constructors; Clone vs timeout-config capture for batch)
unknowns: whether frozen external tests reference AllowAll/::new (grep shows only in-file tests)
