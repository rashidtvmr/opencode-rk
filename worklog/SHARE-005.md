# SHARE-005 — deterministic share-sync transport policy

## Claim
Own `crates/sessions/src/share_policy_lane.rs` + `share_policy2_lane.rs` (SHARE-005 primary + fallback twins; task-card `crates/share/src/policy.rs` never existed — no `opencode-rk-share` crate). Pure policy: at-most-once per seq, 2xx→Sent / 429-5xx→bounded Retry / other-4xx→Abort, explicit endpoint, delete confirmation. No test/lib.rs edits this session. (`src/share_policy.rs` visibility enum is unrelated — naming collision noted for integrator.)

## Source evidence (pin 95daf90670b7c039c436c85537da5fbfe2205b41)
- `packages/opencode/src/share/share-next.ts:206-359`: upstream create/sync/delete (create/delete require success; sync logs HTTP≥400 and drops batch). Deliberate deviation: retain-and-retry, never drop.
- `packages/opencode/test/share/share-next.test.ts:84-224`: transport matrix mirrored in T01..T04.
- `packages/enterprise/src/routes/api/[...path].ts:29-155`: remote route surface treated as transport peer only.
- `sharing-ownership-gap.json` `enterprise-share-http-and-support-admin`; legacy-vs-org auth/failure policy unresolved; `disc-003 opencode.sharing` (partial): legacy `/api/share` vs org `/api/shares` must reconcile before parity.
- Classification: partial upstream, bounded deviation.

## Observed scenario
- Both twins expose `SyncBatch{share_id,base_seq,records}`, `SyncOutcome=Sent|Retry{backoff_ms}|Abort{reason}`, `DeleteOutcome=Deleted|NotFound|Retry`, `SyncPolicy{max_retries=3,base_backoff_ms=500}`, `classify(status,attempt)` (2xx Sent; 429/5xx Retry `base*2^attempt` cap 30_000, past max→Abort; other 4xx Abort), `decide_delete` (2xx Deleted / 404-on-delete NotFound / 429-5xx Retry / other 4xx terminal refusal), `Endpoint=Legacy|Org` explicit (missing→`Abort{endpoint-unspecified}`), `should_send(last_acked,base)` iff `base>last_acked`, `request_target/decide_create/log_event`. Twins differ by 1 doc line only.
- `#[path]`-included by `tests/share_policy_lane.rs` / `share_policy2_lane.rs`; NOT wired into `lib.rs`.

## Target boundary
- Pure fns, no network/DB/clock/thread/global; caller owns batch + schedules retries from `backoff_ms`.
- Retry state two u64 seq markers + one u32 attempt counter per share; policy inspects no payload bytes (size enforced by SHARE-001/002 caps).
- Secrets never enter policy (no secret params); logs status/seq/endpoint only; Debugs redact payloads.

## Tests (frozen, verify-only rerun)
- Suites: `tests/share_policy_lane.rs` + `tests/share_policy2_lane.rs` (T01 happy / T02 retry mapping+cap / T03 abort+endpoint / T04 delete+create semantics / T05 safety+determinism), 5 each.
- GREEN (2026-09-16, `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`, rev `248f519` + workdir dirt, serial, `timeout 120 rtk`):
  `share_policy_lane` 5/5, `share_policy2_lane` 5/5 (each `cargo test -p opencode-rk-sessions --test <suite>` EXIT=0).
- Frozen hashes (sha256): tests `804bca15…` / `47ae926f…`; src `10bbb9fb…` / `0345fd4f…`. No frozen test edited.
- RED history: SHARE-001.md:186-217 — `classify` 2xx→Abort mutation gives 4/5 (T01 RED only) on EACH twin, restored byte-identical GREEN.

## Decisions
- No code change: implementations GREEN; dedicated worklog only. Twin dedup (which copy is canonical) is integrator call.

## Remaining unknowns
- `share_policy.rs` (visibility enum, unrelated) vs `share_policy_lane.rs`/`share_policy2_lane.rs` naming collision — integrator unifies.
- `crates/share` acceptance mapping pending controller allowlist.
