# D1 — Real discovery + auth end-to-end test (TEST-ONLY)

- Package: D1-REAL-DISCOVERY-AUTH
- Base SHA: 259570ba7eb59a2b39b4cce75cd55eda8fc24336
- Branch: D1-real-discovery-auth (clean worktree)
- Owned paths:
  - `crates/cli/src/daemon_client.rs` (append `#[cfg(test)] mod d1_real_discovery_auth` only)
  - `worklog/D1-REAL-DISCOVERY-AUTH.md`
- No production logic changes, no Cargo manifest changes.

## Evidence (pinned canonical 259570ba)

- `crates/cli/src/daemon_client.rs:706` `pub fn discover_from_path(path,&u32,impl Fn(u32)->bool) -> Result<AuthenticatedDescriptor, DescriptorReject>` — module-internal; test module appended in same file sees it.
- `crates/server/src/daemon.rs:299` `publish_backend_descriptor_with_auth(data_dir, SocketAddr, String)` — current pid, create-new 0600, atomic rename; `DaemonPaths::for_data_dir` → `data_dir/runtime/...descriptor`.
- `crates/server/src/daemon_auth.rs:38-64` `DaemonAuth::mint()` (64-hex), `.token()`.
- `crates/server/src/lib.rs:170-181` `AppState { sessions, catalog }`, `router_with_auth(AppState, Option<DaemonAuth>)`.
- Auth policy: missing bearer → 401, wrong bearer → 403 (`daemon_auth.rs:5-6`).
- App-state construction pattern copied from frozen `crates/server/tests/discovery_auth_red.rs:22-34` (`Storage::open_in_memory`, `SessionService::new(Arc)`, `Catalog::default()`).
- CLI deps (crates/cli/Cargo.toml): axum/tokio/server/sessions/storage/catalog/serde_json present; **tempfile NOT a CLI dep** → disposable dir built manually under `std::env::temp_dir()` keyed by pid+test name, removed at end. No user paths touched.
- Raw-HTTP requirement (no tower fake): tokio `TcpListener` 127.0.0.1:0, `axum::serve(...).with_connection_shutdown`? No — plain `axum::serve(listener, app).with_graceful_shutdown(oneshot)`, spawned and awaited.

## Test

`d1_real_discovery_auth::real_discovery_auth_end_to_end` (tokio):
1. temp data dir → `DaemonAuth::mint()` → bind `127.0.0.1:0`, real addr.
2. `router_with_auth(disposable AppState, Some(auth))` served on that addr.
3. `publish_backend_descriptor_with_auth(data_dir, addr, auth.token().to_owned())`.
4. `symlink_metadata` uid (MetadataExt), mode assert 0o600.
5. `discover_from_path(&paths.descriptor, uid, |p| p == std::process::id())` → assert origin+token match minted.
6. bounded raw TCP (64KiB cap, `tokio::time::timeout`, `Connection: close`):
   - discovered bearer `/api/sessions` → 200
   - no header → 401
   - distinct minted wrong bearer → 403
7. shutdown oneshot send + await join; remove temp root.

## Gate

`cargo test -p opencode-rk-cli d1_real_discovery_auth --offline --locked -- --nocapture`

Result: see commit (recorded below).

## Review fix (successor of e23d9953f153183fc9f456b9f4937e79095ad97b)

- Review blocker: temp root was `temp_dir()/d1_real_disc_auth_{PID}_t01` — PID plus constant
  suffix. Duplicate invocation, concurrent race, or stale reuse after a crashed run could share
  or inherit a directory. Test-only fix, no production change.
- Collision: module-static `TEMP_SEQ: AtomicU64`, `fetch_add(1, Ordering::Relaxed)`; path is
  `d1_real_disc_auth_{PID}_{seq}`. No random, no network, no new dependency. Created with
  `std::fs::create_dir` (fails on `AlreadyExists`, never `create_dir_all`), bounded 64-attempt
  retry skips same-PID-same-seq stale leftovers from crashed processes.
- Cleanup: `OwnedTempRoot(PathBuf)` guard; Drop runs `remove_dir_all` on the path it exclusively
  created and owns (panics ignored; no remove on any foreign path, no removal of the bare temp
  base). Covers assertion panics via unwind. Final explicit `remove_dir_all` removed; cleanup is
  the idempotent guard. Graceful shutdown unchanged: `tx.send(())` + bounded 5s await on
  `serve.join()`.
- Unchanged: real `discover_from_path`, descriptor 0600/current-UID/current-PID asserts, live
  axum TCP, valid 200 / missing 401 / wrong 403, 5s I/O timeout, 64KiB response cap.
- Gate rerun (this successor): focused command above, jobs 2, threads 1, executed twice serially
  to prove reusable fresh allocation per run; results recorded in successor commit.
  - Run 1: exit 0. `daemon_client::d1_real_discovery_auth::real_descriptor_discovery_and_bearer_enforcement`
    ok on both bin test targets (`oc2`, `opencode-rk`), `test result: ok. 1 passed` x2. No
    `d1_real_disc_auth_*` leftovers in `$TMPDIR`.
  - Run 2 (serial, same instance): exit 0, identical: 2x ok, no leftovers. Proves per-invocation
    unique allocation (seq 0 each process, fresh create_dir succeeds because prior root was
    removed by guard) and idempotent guard cleanup.
