# V2-DISC-101 — native discovery identity contract

State: **CANDIDATE**; runtime RED/freeze/acceptance pending.

The frozen evaluator supersedes the old three Rust assertions: a server cannot
read a client-local descriptor, valid Bearer authentication remains valid
regardless of forged PID, and a live foreign PID must be rejected before API,
provider, or store effects. Stale and empty descriptors make zero contacts.

Evidence: pinned upstream `95daf90670b7c039c436c85537da5fbfe2205b41`,
`packages/cli/src/services/daemon.ts`; current Rust
`crates/server/src/daemon.rs:136-207`, `crates/server/src/daemon_auth.rs:150-175`;
native callers `crates/cli/src/chat.rs`, `daemon_client.rs`, and `tui_entry.rs`.

The fixture runs fresh disposable `VALIDIDENTITY`, `STALEPID`,
`LIVEFOREIGNPID`, and `EMPTYTOKEN` cases through the installed native caller,
with attested absolute `OC2_TEST_BINARY`, `NATIVE_LIBRARY`, and `BUILD_JSON`
(native release, source/binary/library hashes). It owns its loopback server,
one thread, PTY, process groups, bounded 80x24 capture/wire/request limits,
and cleanup. No Cargo/build/runtime/network gate was run by this source-only
owner. Integrator commands:

```text
git diff --check
python3 -m py_compile tests/e2e/discovery_identity.py
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-server --test discovery_auth_red -- --nocapture
```

Base `1bd286a9a4ac1193185a11d8443f68a8adc50b21`; exact changed paths are this
worklog, the Rust test, and `tests/e2e/discovery_identity.py`. Historical
candidate `87b01e4e1bb500abf06ed6d3637329aa4c310563` remains read-only salvage.
