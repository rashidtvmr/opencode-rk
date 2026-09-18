# SEC-app-policy — PAR-006 app-level policy decisions

Task: PAR-006 app-level policy decision types in `crates/security/src/app_policy.rs`.
Commit: `5af7884cf7637c0760d985da7a03f2f99ccd0c78`.

## Claims / source evidence

- Broker baseline-first: `crates/security/src/lib.rs:202-223 authorize` —
  baseline `decide()` runs first; permission rules only narrow `Allow`;
  mandatory baseline stands regardless of `*`.
- Mandatory gates: `lib.rs:252-335` (`authorize_file/process/sql`) —
  secret deny, system write deny, delete/outside-root/sql-structure human gates,
  destructive `Deny`.
- Star semantics: `star_proof.rs:85-88` global `*` covers everything at
  permission layer, but broker tests (`lib.rs:688-772`
  `star_cannot_read_secrets/write_system/bypass_human_gates/run_destructive`)
  prove baseline holds.
- Bypass in executor (untrusted data, not bypassed here): `crates/tools/src/executor.rs:130`
  `Command::new("bash").arg("-c")` with no broker call.
- Sandbox disclaimer: `sandbox.rs:1` "Policy only, no Landlock syscalls".
  This task must not claim OS sandbox: `require_os_sandbox` fails closed.

## Observed scenario

- File absent before task (`glob **/app_policy.rs` nil, per prior AUD-006 note).
- TDD RED: wrote 15 in-file unit tests, compiled `mod app_policy` via a
  temporary appended `pub mod` line, ran GREEN-implementation with the grant
  path stubbed → 5 grant tests failed as expected
  (`valid_grant_satisfies_human_gate_once`, `replay_invalid`,
  `stale_policy_version_rejected`, `expired_grant_rejected`,
  `local_only_remote_denied`), 10 passed. Then implemented grant path → 15/15.
- Full lib suite with module wired: 132 passed, 0 failed
  (`cargo test -p opencode-rk-security --lib`).
- Temporary `pub mod app_policy;` line in `lib.rs` removed after each run.
  `git status`: only `?? crates/security/src/app_policy.rs`; `lib.rs`
  byte-identical to HEAD. Integrator must add the `pub mod` wiring line.

## Target boundary (owned file only)

- `crates/security/src/app_policy.rs` (~580 lines, `#![forbid(unsafe_code)]`).
- Types: `OperationDigest` (FNV-1a-64 over canonical intent encoding),
  `Scope` (workspace/session/requester/expiry/policy-version; `*`/empty
  rejected in `Scope::new`), `Grant` (approval+digest+scope+`local_only`),
  `ExpectedScope` (explicit `now`, no wall-clock dependence),
  `AppDecision` (Allow/Deny/HumanOnly), `PolicyDeny` (Expired,
  StalePolicyVersion, DigestMismatch, ScopeMismatch, Replay, LedgerFull,
  Mandatory, RemoteOrigin, UnsupportedSandbox, InvalidScope),
  `GrantLedger` (bounded single-use, 4096 cap, overflow fails closed).
- `decide()`: baseline-first. Broker `Allow`→`Allow`. Broker `Deny`→terminal
  `Deny` even with a digest-matching grant under `*` (test
  `mandatory_deny_not_lifted_by_matching_grant`). Broker `RequireHuman`→grant
  must satisfy `covers()` and single-use `consume()`; expired/stale→`Deny`
  (fail-closed); wrong digest/scope→stay `HumanOnly`; replay→`Deny`.
- `enforce_local_only()`: local-only grant from non-local origin→`Deny`.
- `require_os_sandbox()`: always `Err(UnsupportedSandbox)` for `None` and any
  named backend — no regex/prompt masquerading as isolation.

## Tests (frozen intent)

15 in-file tests incl. `wildcard-cannot-bypass` (star+matching grant vs
`.env` read; star+delete without grant) and `replay-invalid` (second use
denies). Plus stale/expiry/digest/scope/local-only/sandbox-fail-closed/
digest-sensitivity/serde/ledger-overflow/baseline-allow/star-human-gate.

## Decisions

- Grant defects split: expired/stale/invalid-scope → `Deny`; wrong
  digest/scope → `HumanOnly` (still needs real human, not denial of op).
- Failed `covers()` never consumes the grant; replay check before consume.
- `ponytail:` FNV-1a is binding-only, not collision-resistant; upgrade to
  blake3 when a new Cargo dependency is approved.
- No new deps: serde/serde_json/thiserror/contracts already in
  `crates/security/Cargo.toml`; `std::time::SystemTime`, no chrono.

## Remaining unknowns / integration notes

- `lib.rs` wiring NOT committed (owned-file boundary): integrator adds
  `pub mod app_policy;` to the `pub mod` block (`lib.rs:3-20`).
- Verifier command with wiring: append the line, run
  `timeout 120 env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-security --lib`,
  then restore.
- Executor (`executor.rs:130`) still bypasses broker — out of scope for this
  lane; needs its own enforcement slice.
