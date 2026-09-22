# BASE-004 scratchpad (implementation lane)

Claim: BASE-004 held by `ses_f38227d01ffeptQrUFzwnKG3kn` (`in-progress`).
Ownership: `crates/server/src/daemon.rs`, this scratchpad, own ledger row.
Frozen test: `crates/server/tests/daemon_long_path.rs`
SHA-256 `a3402c6ddd2cf4d14823511e1845cedc37bda1a2bd728ae975633673c904101d`
(unchanged, never edited).

## Source evidence

- `crates/server/src/daemon.rs:136-146`, `DaemonPaths::for_data_dir`: pid and
  descriptor stay under `<data_dir>/runtime`; socket now derives via
  `short_socket_path`.
- `crates/server/src/daemon.rs:148-275`, new derivation + gate:
  `MAX_SOCKET_PATH_BYTES=104`, `short_root` (`/private/tmp` macOS,
  `/tmp` other unix, `temp_dir` non-unix), `short_socket_path`,
  `fnv1a64` x2 bases, `uid_tag`, `normalized_key`, `ensure_socket_parent`,
  `is_managed_leaf`, `mode_is_private`.
- `crates/server/src/daemon.rs` bind section: `PidLock::acquire` runs before
  `ensure_socket_parent`, before stale-socket removal, before
  `UnixListener::bind`. Lock authority unchanged.
- Reused helpers: `owner_uid` (unix MetadataExt), `current_uid`
  (Linux `/proc/self/status` euid, Darwin `/usr/bin/id -u`, non-unix
  fail-closed `Err`). No new imports, deps, or Cargo edits.
- Callers needing no change: `crates/cli/src/main.rs` serve binds
  `daemon_paths.socket` + `daemon_paths.pid`; descriptor discovery reads
  `paths.descriptor` under the data dir.
- Frozen RED: `crates/server/tests/daemon_long_path.rs:10-44`.
- Existing suites: `crates/server/tests/web_singleton_lock.rs:13-74`,
  `crates/server/tests/daemon_auth_api.rs`, `daemon.rs` unit tests.

## Design

Socket = `<short_root>/rk-<euid>/rk-<hex128>.sock` where hex128 is two
FNV-1a 64-bit digests (distinct offset bases) over
`euid || 0xff || normalized-path-bytes`. Normalization walks
`Path::components`: prefix/root kept, `.` skipped, `..` pops, duplicate
separators collapsed. Same canonical or logically equivalent spelling maps
together; different data dirs map apart. No `HashMap` random state, no wall
clock, no PID, no secret, no basename, no environment reads: `TMPDIR`
deliberately unused in both derivation and root.

## Security / failure / lifetime semantics

- Parent gate in `bind`, after lock held: `symlink_metadata` must show a
  real directory; symlink or non-directory fails closed with `DaemonError::Io`.
  Missing parents are created then chmod 0700 on unix. Unix re-stats the
  parent: owner must equal caller euid. A pre-existing unrelated directory
  keeps its mode; only our managed `rk-<uid>` leaf directly under the short
  root is additionally refused when it lost private mode (group/other bits).
- No broad deletion: only the exact socket file is removed, only after the
  PID lock is held, so a live owner's socket is never deleted by a rival
  (`acquire` returns `AlreadyRunning` first).
- Drop semantics unchanged: `PidLock::drop` unlocks; `SingletonDaemon::drop`
  removes only its socket file.
- Auth untouched: descriptor schema/pid/liveness/loopback/bearer validation
  unchanged; no new token, header, or router behavior.
- Resource bounds: derivation allocates one small key vector; no queues,
  threads, retries, or unbounded reads. `debug_assert` guards the
  `< 104` bound on unix-length paths in dev builds; the frozen test asserts
  it in all builds.
- Cross-platform: unix paths bounded (`/tmp` + `rk-N` + 40-char name is far
  below 104). Non-unix uses `temp_dir` and skips unix-only owner/mode
  checks; `current_uid` fails closed to the `unknown` tag. `forbid(unsafe_code)`
  preserved: no unsafe, no shell strings, no env reads.

## Unit tests added inside daemon.rs (frozen test untouched)

- `socket_paths_isolated_and_bounded`: determinism, `./` equivalence,
  profile-a vs profile-b distinctness, long-path bound, pid/descriptor
  under data dir, socket outside data dir.
- `socket_parent_gate_refuses_symlink_not_dir`: symlink parent and
  file-as-parent both refused.

## Verification

- RED reconfirmed before edit:
  `cargo test -p opencode-rk-server --test daemon_long_path -- --test-threads=1`
  => 1 failed (socket 225 bytes vs bound).
- GREEN after:
  `cargo test -p opencode-rk-server --test daemon_long_path -- --test-threads=1`
  => 1 passed.
- `cargo test -p opencode-rk-server --lib daemon`
  => 23 passed (21 pre-existing + 2 new), 0 failed.
- `cargo test -p opencode-rk-server --test daemon_auth_api -- --test-threads=1`
  => 5 passed, 0 failed.
- `cargo test -p opencode-rk-server --test web_singleton_lock`
  => 1 passed, 1 failed: `web_006_t02` fails at `web_singleton_lock.rs:31`
  with `Descriptor("descriptor predates bearer auth (empty auth_token)...)`.
  Pre-existing on clean tree (verified via `git stash` + rerun: same
  `test failed` before pop), caused by the RC-01 bearer rule vs the legacy
  `publish_backend_descriptor` in that test, unrelated to this lane. Frozen
  file left untouched per contract.
- `cargo check -p opencode-rk-server`: no errors; only pre-existing warnings
  from other crates.
- Frozen SHA re-verified: `a3402c6ddd2cf4d14823511e1845cedc37bda1a2bd728ae975633673c904101d`.
- Scoped lane gate: `tools/lane_gate.py` not present in repo; exact scoped
  cargo targets run instead.

## Remaining seams

- FNV-1a is non-cryptographic (64-bit birthday bound per half); deliberate
  `ponytail` ceiling noted in code: swap for SHA-256 truncation when a
  crypto dependency is approved. Collision consequence is limited: two data
  dirs sharing a socket path serialize on the PID lock and the second gets
  `AlreadyRunning`, never silent cross-talk.
- `web_006_t02` (see above) needs its owning lane to reconcile the legacy
  no-token publish helper with the RC-01 bearer rule; out of BASE-004
  authority.
- No two-user same-machine adversarial test run here; isolation rests on the
  euid tag + owner check, reviewed but not live-attack-tested.
