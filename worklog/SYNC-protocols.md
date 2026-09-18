# Worklog: SYNC-protocols (PAR-009 app_protocols.rs)

## Claim
Versioned protocol envelope types for SDK/sync/remote-proxy/headless wire
boundary: `WIRE_VERSION` const, `RequestId`/`DeviceId`/`SessionId`,
`OpDigest`, `EventCursor` + `check_cursor`, frame caps
(`MAX_FRAME_BYTES`/`MAX_QUEUE_ITEMS`/`MAX_QUEUE_BYTES`), `IdempotencyKey` +
`idempotency_key` helper, `classify` ephemeral-vs-durable, `FrameQueue`,
`encode`/`decode` fail-closed. Real Rust, `forbid(unsafe_code)` + serde,
16 unit tests (oversized/malformed fail-closed + cursor resync-required).

## Source evidence (HEAD 5af7884)
- `crates/contracts/src/lib.rs:14` — `WIRE_SCHEMA_VERSION: u16 = 1`;
  `:270-308` — `EventCursor(u64)`, `VersionedEnvelope::validate_version`.
- `crates/server/src/event_bus.rs:1-30` — bounded fan-out
  `ServerEvent::{SessionCreated,MessageAppended,ToolExecuted,
  PermissionRequested,Shutdown}` + `EventKind`.
- `crates/server/src/workspace_proxy.rs:1-22,104-188` — caps
  4 MiB queue / 256 items / 256 KiB item; `route` allowlist.
- `crates/server/src/sdk_client.rs:1-13` — typed client, caller-owned
  `HttpPort`, no socket I/O, 30 s timeout cap.
- `crates/server/src/event_stream.rs:23-54` — 1 MiB frame / 64-frame
  per-subscriber envelope (transport-level; app caps here match
  proxy/sync 256 KiB instead).
- `crates/server/src/remote_sync.rs:11-20` — 2 s/30 s backoff,
  256-item / 4 MiB / 256 KiB queue caps mirrored.
- `crates/server/src/sync_log.rs:10-15` — 4096 events / 32 projectors /
  64 KiB payload (store-level; app frames stay at 256 KiB wire cap).
- `crates/contracts/src/events.rs:1-149` — `DomainEvent` taxonomy used to
  derive the durable/ephemeral classifier list.
- `tasks/completion/parity.json:12` — PAR-009 journey + 5 test bullets.
- `sources/completion/audits/AUD-010.json:48` — notes `app_protocols.rs
  absent` (gap this lane fills at the types level; runtime wiring stays
  out of scope for the owned file).

## Observed scenario
`app_protocols.rs` did not exist; contracts had version + cursor but no
request/device/op-digest/idempotency/frame-queue/classifier boundary
shared by the four PAR-009 callers.

## Target boundary
- Owned file only: `crates/server/src/app_protocols.rs`. No `lib.rs`
  wiring (integrator-owned per AGENTS.md §5; lane_gate pre-wires shared
  files before fan-out). Scratch harness at
  `/tmp/opencode/proto-verify` includes the file by `#[path]` so the
  repo tree is untouched.
- Pure sync, no I/O/clock/threads/globals/logging. Errors carry variant
  names + one numeric version only (secret-free by construction).
- Caps mirror proxy/sync (256 KiB / 256 / 4 MiB); `WIRE_VERSION = 1`
  tracks `WIRE_SCHEMA_VERSION`.

## Tests (frozen in-file `#[cfg(test)]`, 16 cases)
Roundtrip, wrong-version fail-closed, oversized frame fail-closed
(encode + decode + queue push), malformed ID token table, malformed
wire bytes (non-UTF8/truncated/array/wrong-ID), digest hex bounds,
idempotency determinism + malformed keys, stale/future/empty cursors
(resync-required + zero-is-bad), cursor `next` saturation, durable list
vs ephemeral deltas + unknown-future-ephemeral, frame header checks,
queue item-cap + byte-cap fail-closed (queue byte-identical), error
secrecy (no input echo).

## Decisions
- `valid_token`: first-char alnum, rest alnum/`.`/`_`/`-` (`:` only for
  keys, derived `request:digest` form). Rejects paths/escapes/controls.
- `OpDigest`: 32..=128 hex chars, case-insensitive (128–512-bit digests).
- `check_cursor`: zero → `BadCursor`; behind-retention or past
  `head+1` → `ResyncRequired` (snapshot, never gapped replay).
- `classify` total: 8 durable lifecycle/history types; everything else
  incl. unknown future types → `Ephemeral` (projector can never invent
  durable messages from deltas).
- Durable list excludes `tool.invoked` and `permission.requested`
  (pre-effect intents, not settled state); includes `tool.completed`
  and both permission resolutions.
- `decode`: byte-cap → UTF-8 → JSON → `validate` (version first).

## Verification
- RED: stubbed validators (`if true`) in scratch checkout → 5 tests
  FAILED (`wrong_version`, `malformed_ids`, `malformed_wire_bytes`,
  `digests`, `idempotency`), proving the suite fails for missing
  behavior. Restored afterwards; file hash back to GREEN revision.
- GREEN: `cargo test --manifest-path
  /tmp/opencode/proto-verify/Cargo.toml` → 16 passed, 0 failed.
- `cargo clippy` on harness: only dead-code warnings (expected — file
  not yet wired into `lib.rs` by integrator).
- Full `cargo check -p opencode-rk-server` not run: host at ~6 GiB
  total, other lanes active; scratch harness typechecks + tests the
  exact file without a workspace build.
- `sha256sum crates/server/src/app_protocols.rs` recorded below;
  `git status` shows the file as the only lane-owned untracked path.

## Remaining unknowns
- Integrator must add `pub mod app_protocols;` to `lib.rs` and wire
  callers (sdk/sync/proxy/headless); dead-code warnings clear then.
- Runtime wiring + e2e `tests/e2e/protocol_parity` belong to later
  PAR-009 slices, not this types lane.
