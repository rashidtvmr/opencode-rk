# 004-B backend security review

**Verdict: NEEDS_FIX**  
**Base:** `e3069eee04d9a3414517b2452df803c9c145c9d3`  
**Scope:** Current nine-path backend source review; no product/test edits, builds, tests, Git, UI, DB, or delegation. Not installed `ACCEPTED`.

## Finding

**Cross-session typed-history disclosure.** `crates/storage/src/lib.rs:651-653` finds a tool round by `message_id` without constraining `r.session_id` to the requested session or verifying the linked display row belongs to that round. `crates/storage/src/session_context.rs:42-48` likewise selects preview records by `message_id` alone. Schema at `lib.rs:1621-1622` uses separate foreign keys, not a session-consistency constraint. A malformed/corrupted typed row can link a foreign round's call/output to a tool display row owned by this session; history/provider projection may emit foreign payload, and inspection may disclose it. Add session/round/display ownership checks in both readers; regression-test malformed cross-session linkage. This violates fail-closed isolation; approval blocked.

Other reviewed controls: bearer middleware on production `/api/*`, bounded mutation bodies, typed IDs, system protection, shared turn permit, exclusion bounds/transactions, complete-pair filtering, checkpoint head validation and atomic stale-exclusion cleanup, bounded previews, provider-only opaque checkpoint. Earlier compacted-message include/exclude conflict and unsupported-format 422 corrections confirmed.

## Evidence

No checks run by reviewer. Parent evidence logs report backend 235 tests plus focused targets, storage 122 plus four suites, native 769 tests passing. Terminal candidate journey records exclusion/restart/re-inclusion and durable-history preservation; not installed acceptance.

## SHA-256 manifest

| Path | SHA-256 |
|---|---|
| `crates/storage/src/lib.rs` | `7229fb0b7982b3a532847b0148d922b1f7181e0782dab74243c122e4d667616f` |
| `crates/storage/src/session_context.rs` | `4ca5c8cb967083f4ca5314f7e53b75b7f64d278541cacfc0912df9be07e29db2` |
| `crates/storage/src/session_compaction.rs` | `a3e7a1cf324c3c78a7eeb2d380bf445d9a9e22c2829eba0174408db3f30e65d8` |
| `crates/sessions/src/lib.rs` | `f155e726776e6966012d19564cbd0cf62c03abb43a6b8bec68072a2a98ebe5d5` |
| `crates/server/src/lib.rs` | `d752f35cc8986715ec748d76dd12fba174793dac39456988d19806fbf4c0af53` |
| `crates/server/src/context_http.rs` | `9401349c39adedbeb8e02890c6a0682a63789aa4f4ee18b05ba09dcc59cf528f` |
| `crates/server/tests/session_context_selection.rs` | `df39053148b72b7b29646c5c4f4d061a8abce3166cae9d4196d4947bfb4e400a` |
| `crates/server/tests/context_selection_projection.rs` | `f8a359444450cfa50ac5ced66d3cc4beb7ee70338e33e95977617365a14aaf7e` |
| `crates/server/tests/context_checkpoint_selection.rs` | `728e660b2cf5ad434e342b2062373badf173e468f2480b857e2e77fd1a372111` |
