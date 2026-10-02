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
