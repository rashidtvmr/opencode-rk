# NET-011-rpty — constrained remote PTY session control

Claim: `crates/server/src/remote_pty.rs` implements grant-gated PTY types
with byte/rate/scrollback caps, lease/retention disconnect, restricted
subprocess inheritance marker, hostile-sequence sanitizer. std only, no
process spawn, `#![forbid(unsafe_code)]`.

Source evidence (HEAD 5af7884):
- Task card: `tasks/completion/remote.json:14` (NET-011, paths
  `crates/server/src/remote_pty.rs`). File was MISSING; created.
- Contract: AGENTS.md worker contract, docs/TDD.md RED/GREEN lifecycle,
  docs/SECURITY.md capability policy (human-only grants, bounded resources,
  no ambient env/secrets).
- Style reference: `crates/server/src/remote_sessions.rs:1-120` (grant-like
  routing, `#![forbid(unsafe_code)]`); `origin_check.rs` (pure predicate).

Observed scenario:
- `rustc --edition 2021 --test crates/server/src/remote_pty.rs -o
  /tmp/opencode/rp && /tmp/opencode/rp` → 7 passed, 0 failed.
- Borrowck failure (E0502 on window bookkeeping) fixed by making
  `window_allows`/`window_spend` associated fns taking `&PtyLimits`.
- RED gap closed: expired-but-swept session reported `UnknownSession`
  instead of `LeaseExpired`; added `reaped` tombstone map (bounded by
  `retention_ticks`) so post-lease reconnect names the cause.

Target boundary: OWNED FILE ONLY `crates/server/src/remote_pty.rs`. No
other edits (lib.rs wiring left to integrator).

Tests (7, frozen in-file `mod tests`):
- granted_input_output_and_resize_roundtrip (T01)
- ungranted_and_readonly_cannot_inject_or_start_shell (T02; asserts empty
  pending input + unchanged geometry = absence of side effects)
- subprocess_inheritance_is_restricted_project_scoped (T03; ambient env
  false, no secret passthrough, fs confined, net isolated, human-only)
- disconnect_reconnect_follows_lease_without_orphans (T04; resume inside
  lease, sweep reaps, NO_ORPHAN_PROCESSES marker)
- flood_respects_byte_rate_and_scrollback_budgets (T05; oversize rejected
  whole, rate window refuses then rolls over, scrollback within caps)
- hostile_terminal_sequences_neutralized (T05 marker: predicate +
  sanitizer + end-to-end scrollback clean)
- invalid_resize_and_bounds_rejected (geometry bounds, no partial resize)

Decisions:
- No OS process spawn in module: gateway owns byte buffers only; makes
  no-orphans structural. Real spawn/lifecycle owned by integrator slice.
- Time as explicit `now: u64` ticks: deterministic fixtures, no wall clock.
- Reap tombstones retained `retention_ticks` then forgotten (explicit quota).
- Scrollback evicts oldest-first (explicit quota, never silent user-history
  delete beyond the documented cap).

Remaining unknowns: gateway→real workspace process wiring, auth-token
binding of PtyGrant, multi-device concurrency policy — integrator scope.
