# WEB-006-DESCRIPTOR scratchpad

Claim: WEB-006 claimed by `ses_f37d1ed3affeMszns7VKYpWhOw`, scratchpad `worklog/WEB-006-DESCRIPTOR.md`. Prior row was `completed` (WEB-006-READINESS lane), not a live collision; claim succeeded.
Ownership: exactly one product file `crates/server/src/daemon.rs`; plus this scratchpad and own ledger row. No frozen test, main.rs, daemon_auth.rs, lib.rs, Cargo, controller/verifier/policy/foreign edits.

## HEAD and source evidence

HEAD `67fb61c79886f9569cae5b4b2631f9c2c99381cd` (branch `lane/WEB-006-integration`).
Worktree `/Users/mymac/Projects/opencode-rk-web006-integrate`.

- `crates/server/src/daemon.rs:528-533` `publish_backend_descriptor` delegates to `publish_backend_descriptor_with_auth(data_dir, address, String::new())`, writing `auth_token: ""`.
- `crates/server/src/daemon.rs:376-417` `read_backend_descriptor`: live well-formed endpoint with empty token returns `Err(Descriptor("descriptor predates bearer auth ..."))`, never `Ok(Some)`.
- `crates/server/src/daemon_auth.rs:44-50` `DaemonAuth::mint()` mints 32 bytes from `/dev/urandom`, hex, fail-closed via `AuthError::NoRandomness`.
- `crates/server/src/daemon_auth.rs:54-63` `from_published` rejects empty/malformed; `TOKEN_HEX_LEN` = 64.
- Frozen `crates/server/tests/web_singleton_lock.rs` SHA-256 `ce0802377f00d4460f6d2eea2435aef5fec07aab120d6fdd572840eda8dc6393`; test `web_006_t02_stale_pid_and_descriptor_are_recoverable` publishes via no-token helper then asserts `read_backend_descriptor == Some(published)`.

## Observable contract / failure / lifetime / resource bounds

- Contract: `publish_backend_descriptor(data_dir, address)` mints a real `DaemonAuth` credential via existing same-crate API and delegates to `publish_backend_descriptor_with_auth` with its token, so the descriptor current authenticated readers consume is `Some` immediately. `publish_backend_descriptor_with_auth` preserved for owners that already hold their credential.
- Failure: mint failure maps into existing typed `DaemonError` without leaking raw entropy/source details. Legacy empty-token descriptors read from disk still rejected immediately (`Err`), unchanged. No weakening of `read_backend_descriptor`, `DaemonAuth::from_published`, middleware, bearer checks.
- Lifetime: credential lives in the published descriptor file (owner-checked, 8KiB-capped) as before; no new persistence.
- Resources: one mint (single 32-byte urandom read) + one bounded descriptor write per publish; no new deps, network, thread, shell, unbounded work.
- Security: never fixed/test/env/predictable token; token not logged/returned outside existing descriptor contract.

## Tests

- Frozen `web_singleton_lock.rs` SHA-256 `ce0802377f00d4460f6d2eea2435aef5fec07aab120d6fdd572840eda8dc6393`, zero test edits (verified identical before and after).
- RED (compiling, pre-implementation): `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test web_singleton_lock web_006_t02_stale_pid_and_descriptor_are_recoverable -- --exact --test-threads=1` exit 101, `FAILED`, panic at `web_singleton_lock.rs:31:45` unwrapping `Err(Descriptor("descriptor predates bearer auth (empty auth_token); refusing to attach"))`. Log `/private/var/folders/.../T/opencode/red.log`. Confirms cause: no-token `publish_backend_descriptor` writes `auth_token: ""`, reader rejects.
- GREEN (post-implementation, same exact command): exit 0, `1 passed; 0 failed`. Log `/private/var/folders/.../T/opencode/green-t02.log`. (Test binary prints `kill: illegal process id: 4294967295` from pid_alive probing the u32::MAX stale pid; harmless, test still ok.)
- `web_singleton_lock` 2/2 GREEN (t02 + t04), exit 0. Log `green-singleton.log`.
- `daemon_auth_api` 5/5 GREEN (rc01_t01-t05: 401/403/state/public/auth), exit 0. Log `green-auth.log`. Bearer auth unweakened.
- `daemon_long_path` 1/1 GREEN, exit 0. Log `green-longpath.log`.
- `cargo test -p opencode-rk-server --lib daemon` 24/24 GREEN (15 daemon incl `desc_stale_empty_token_errors`, `desc_stale_malformed_token_errors`, `desc_stale_valid_token_attaches` + 5 daemon_auth incl `rc01_t05_published_restore_rejects_legacy_blank` + 4 acp_session daemon-filter matches), exit 0. Log `green-lib.log`. Legacy-empty-token negatives remain GREEN in-file; no new test needed.
- `cargo check -p opencode-rk-server` exit 0, 10.59s, 3 pre-existing warnings (unused import Duration x2, thiserror Error, dead fields), zero errors. Log `check.log`.

## Decisions

- Mint inside `publish_backend_descriptor` via `crate::daemon_auth::DaemonAuth::mint()`, delegate token to `publish_backend_descriptor_with_auth`. Reuses existing same-crate API, preserves the with_auth variant for owners holding credentials (main.rs:728-729 unchanged caller).
- Mint failure maps to `DaemonError::Descriptor("cannot mint daemon bearer credential")`: existing typed error, no raw entropy/source detail leaked, no new `From<AuthError>` conversion (avoids widening error API).
- Doc updates: no-token helper now secure-by-default; with_auth doc warns fresh publication must use the minting helper; reader rejection of legacy empty-token descriptors unchanged.
- Scratchpad `worklog/WEB-006-DESCRIPTOR.md` created after successful claim; owned-file-only diff confirmed (`daemon.rs` + scratchpad + ledger row).

## Remaining unknowns

- None for this lane. No release acceptance claim; long-path/readiness fixes verified unaffected via their suites above.
