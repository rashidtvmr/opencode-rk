# SHARE twins disposition (READ-ONLY survey, no code touched)

Scope: `crates/sessions/src/share_*.rs` vs `*_lane.rs` twins, plus
`share_policy_lane` vs `share_policy2_lane`. Frozen tests untouched.
`share_merge.rs` / `share_queue.rs` owned by redaction writer — not touched.

## 1. Workdir `git diff --stat` (uncommitted, pre-existing)

| file | stat | kind |
|---|---|---|
| `share_merge.rs` | +13/-2 | redaction: manual `Debug` for `ShareRecord` (`payload_len`) |
| `share_queue.rs` | +13/-5 | redaction: manual `Debug` for `ShareEvent` (`value_len`) |
| `share_queue_lane.rs` | +1/-4 | rustfmt-only reflow |
| `share_store.rs` / `share_store_lane.rs` | +5/-5 each | rustfmt-only, mirrored |
| `share_policy_lane.rs` / `share_policy2_lane.rs` | +1/-3 each | rustfmt-only, mirrored |
| `share_merge_lane.rs`, `share_enterprise.rs`, `share_enterprise_lane.rs` | clean | — |
| `lib.rs` | +3 | `part_events`, `runner`, `tui_info_panel` (unrelated) |

## 2. Twin comparison (canonical vs lane, `diff | grep -c '^[<>]'`)

| pair | twin diff lines | symbols | test imports | `lib.rs` | re-export? |
|---|---|---|---|---|---|
| `share_merge` / `merge_lane` | 130 | ALL renamed: `RecordKind`→`LaneRecordKind`, `ShareId`→`LaneShareId`, `ShareSecret`→`LaneShareSecret`, `ShareRecord`→`LaneShareRecord`, `MergeOutput`→`LaneMergeOutput`, `ShareError`→`LaneShareError`, `MAX_*`→`LANE_MAX_*`, `validate`→`validate_lane_secret`, `merge_share_records`→`merge_lane_records`, `apply_sync`→`apply_lane_sync` | `#[path]` includes; canonical test uses canonical names (`share_merge.rs:6-10`), lane test uses `Lane*` names (`share_merge_lane.rs:8-12`) | `pub mod share_merge` wired; lane NOT wired | NO — every public symbol renamed; glob re-export exposes wrong names, breaks frozen lane imports |
| `share_queue` / `queue_lane` | 62 | ALL renamed: `DataKey`→`LaneDataKey`, `ShareEvent`→`LaneShareEvent`, `QueueCaps`→`LaneQueueCaps`, `QueueError`→`LaneQueueError`, `QueueStats`→`LaneQueueStats`, `CoalescingQueue`→`LaneCoalescingQueue`; same fn set (`new/push/drain/requeue/finalize/len/bytes/filtered/evicted/stats`) | `#[path]` includes; canonical vs `Lane*` imports | `pub mod share_queue` wired; lane NOT wired | NO — same reason as merge |
| `share_store` / `store_lane` | 4 (2 comment lines) | IDENTICAL (`ShareId`, `ShareSecret`, `ShareMeta`, `StoreError`, `ShareStore`, `MAX_SHARES`, `MAX_URL_BYTES`) | `#[path]` includes; identical import lists | NEITHER wired | YES technically (identical API) but pointless — `#[path]` tests never resolve via `lib.rs`; keep-both |
| `share_enterprise` / `enterprise_lane` | 32 | IDENTICAL names (`EnterpriseOp`, `BoundaryError`, `partition`, `refusal_reason`, `EnterpriseBoundary::authorize`); impl differs: canonical `thiserror::Error` + NO `forbid(unsafe_code)`; lane `forbid(unsafe_code)` + manual `Display`/`std::error::Error` (same message string) | `#[path]` includes; identical import lists | NEITHER wired | NO in practice — names match but shim changes dependency surface (thiserror vs std) and drops lane's `forbid(unsafe_code)`; keep-both |
| `policy_lane` / `policy2_lane` | 2 (1 doc line) | IDENTICAL (full transport-policy API: `ShareId`, `ShareRecord`, `SyncBatch`, `SyncOutcome`, `DeleteOutcome`, `Endpoint`, `SyncPolicy`, `should_send`, `request_target`, `decide_delete`, `decide_create`, `log_event`) | `#[path]` includes; identical import lists | NEITHER wired; NO canonical owner exists | YES technically (file shim `#[path]`-re-export preserves test mod name) but WRONG direction — no canonical to shim to; see §3 |

Drift note: canonical `share_merge`/`share_queue` test headers claim "NOT wired
into `lib.rs`", but `lib.rs:23,25` wires both (`share_policy` too). Harmless —
no test uses a crate path — but headers are stale. Integrator call.

## 3. Policy collision

`share_policy.rs` (wired, `lib.rs:24`) is an UNRELATED visibility enum
(`Visibility::{Private,Link,Workspace}`, `parse_visibility`,
`visibility_label`) with its own crate-path test (`tests/share_policy.rs`).
`share_policy_lane.rs` / `share_policy2_lane.rs` are SHARE-005 transport policy
(two lanes, same API, 1-line doc drift). Merging any lane into `share_policy`
is a type collision, not a dedup.

## 4. Recommendation: keep-both everywhere; rename, never shim, never delete

- merge, queue: keep-both (Lane* rename makes shim impossible).
- store: keep-both (shim valid but purposeless under `#[path]` tests).
- enterprise: keep-both (shim valid for names, invalid for dep/`unsafe` surface).
- policy_lane vs policy2_lane: keep-both now; integrator RENAMES fallback
  (e.g. `share_transport_fallback`) — never shim one to the other (no canonical
  owner) and never fold either into `share_policy` (unrelated visibility type).
- No deletions/edits by this lane (redaction writer owns merge/queue).
