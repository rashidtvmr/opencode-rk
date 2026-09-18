# NET-013-revoke worklog

Claim: revocation types in `crates/server/src/remote_revocation.rs`, std only,
`#![forbid(unsafe_code)]`.

Source evidence (HEAD 5af7884):
- `crates/server/src/remote_ledger.rs:2` — `#![forbid(unsafe_code)]` pattern,
  bounded const (`MAX_LEDGER_ENTRIES:39`), typed errors; followed same idiom.
- `crates/server/src/origin_check.rs:1-10` — pure predicate style precedent.
- No existing revocation module; new file, no callers wired (single-file lane).
- Card NET-013 via `tools/completion_plan.py --card NET-013` (T01..T05 map to
  the six tests below; isolation case added for journey "without disrupting
  unrelated paired devices").

Observed scenario: RED run compiled, 0/6 passed (missing-behavior failures:
`UnknownDevice` on enroll paths, `Ok` on reuse, `UnknownOp` vs `StaleEpoch`);
frozen test hash `8e13cddf` (post GREEN-refactor; RED log `/tmp/opencode/red.log`,
GREEN log `/tmp/opencode/green.log`, final `/tmp/opencode/final.log`).

Target boundary: epoch-bump on revoke/rotate invalidates streams + queued ops
via reauthorize-at-execution; refresh-reuse/cloned/expired rejected;
re-enroll requires fresh (unconsumed, post-revocation-watermark) challenge;
`MAX_REVOCATION_PROPAGATION_SECS=30` bound + `propagation_deadline()`.

Tests (in-file `#[cfg(test)]`, all real behavior vs real impl):
- revoked_closes_streams_and_rejects_new_commands (T01)
- queued_ops_reauthorized_post_epoch (T02)
- refresh_reuse_cloned_and_expired_rejected (T03)
- reenroll_needs_fresh_challenge (T04)
- propagation_bound_documented (T05)
- revoke_isolated_to_device (journey isolation)

Decisions:
- Queued ops retained-but-stale on epoch bump (not dropped): `execute_queued`
  returns `StaleEpoch`/`Revoked`, no replay ambiguity for op-id holders.
- One-time challenges tracked by issuance seq + per-device revoke watermark;
  consumed tokens removed, stale tokens rejected with `NeedsFreshPairing`.
- Bounds: devices 64, streams 256, queued 256, challenges 128, payload 4KiB,
  credential TTL 3600s, propagation 30s. `Display` never renders secrets.

Remaining unknowns: integration with actual socket layer / audit + UI surface
(T05 UI half) belongs to integrator slice; file has no `mod` wiring (lane
owns single file only).
