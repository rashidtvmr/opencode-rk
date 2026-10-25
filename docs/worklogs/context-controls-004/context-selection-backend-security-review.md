# 004-B backend security review

**Verdict: APPROVE**

**Base:** `e3069eee`
**Scope:** Current nine-path context-selection backend diff only. Source review; no edits to product/tests, no build/test, Git, UI, DB, or install verification. Supersedes preliminary review.

## Findings

No remaining security defect found in reviewed delta. The prior ineffective per-message include is fixed: `context_http::select` checks compaction boundary for both include and exclude before storage mutation, returning conflict for compacted messages; only whole-checkpoint selection proceeds. Existing fork-backed sessions pass `sessions.get`; storage-unavailable context selection returns explicit unsupported-storage 422, never success/persistence.

Security controls verified in source: authenticated router applies bearer middleware to `/api/*`; route-level selection bodies cap at 1 KiB; IDs parse to typed `SessionId`/`MessageId`; SQL uses parameters and session-owned row predicates; system items are rejected; the selection handler acquires the same per-session turn permit as provider assembly and compaction. Storage bounds exclusions to 256, validates stored typed IDs/session ownership, uses immediate transactions, keeps tool call/output filtering atomic, and validates full typed history before applying exclusions. Checkpoint save validates the captured head and upserts/deletes old checkpoint selection within one transaction. Preview inputs have finite page/file/tool/checkpoint bounds; serialized response cap follows serialization but source fields and page counts are explicitly capped. No opaque native checkpoint payload is returned.

## Verification evidence

Source only; not independently executed. Existing evidence: `context-selection-backend-final-fixed-check.log` reports 235 unit tests and context HTTP/checkpoint/projection tests passing before final formatting. `context-checkpoint-storage-final-check.log` reports 122 storage unit tests plus focused suites passing. Parent must rerun required gates on exact formatted source; no installed acceptance implied.

## Current SHA-256 manifest

| Path | SHA-256 |
|---|---|
| `crates/storage/src/lib.rs` | `7229fb0b7982b3a532847b0148d922b1f7181e0782dab74243c122e4d667616f` |
| `crates/storage/src/session_context.rs` | `4ca5c8cb967083f4ca5314f7e53b75b7f64d278541cacfc0912df9be07e29db2` |
| `crates/storage/src/session_compaction.rs` | `a3e7a1cf324c3c78a7eeb2d380bf445d9a9e22c2829eba0174408db3f30e65d8` |
| `crates/sessions/src/lib.rs` | `f155e726776e6966012d19564cbd0cf62c03abb43a6b8bec68072a2a98ebe5d5` |
| `crates/server/src/lib.rs` | `44e9dd5215dac825fb5bc0b4f2503530bfc5bb7519307a14e2035644e69c9783` |
| `crates/server/src/context_http.rs` | `9401349c39adedbeb8e02890c6a0682a63789aa4f4ee18b05ba09dcc59cf528f` |
| `crates/server/tests/session_context_selection.rs` | `df39053148b72b7b29646c5c4f4d061a8abce3166cae9d4196d4947bfb4e400a` |
| `crates/server/tests/context_selection_projection.rs` | `f8a359444450cfa50ac5ced66d3cc4beb7ee70338e33e95977617365a14aaf7e` |
| `crates/server/tests/context_checkpoint_selection.rs` | `728e660b2cf5ad434e342b2062373badf173e468f2480b857e2e77fd1a372111` |

**Disposition:** Source review approved for these hashes only. Not installed `ACCEPTED`; final exact-source test rerun remains parent-owned.
