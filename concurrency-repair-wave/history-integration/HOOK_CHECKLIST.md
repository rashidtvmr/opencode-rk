# History Request State Hook Checklist

Purpose: exact integration map for dropping in `native_history_request_state.rs`
helpers (`reset_binding_locals`, `retire_matching`) into `tui_entry.rs` so that a
late response A after a binding reset never clears or blocks a newer request B.

Reference: `concurrency20/history-request-state-v2/native_history_request_state.rs`
(150 lines, 3 tests passing).

## G0. Binding change sites -- MUST call reset_binding_locals-equivalent

Each site commits a new `binding_generation` and replaces `history_window`.
At each site, after `live = Some(snapshot)` and `history_window = ...new(...)`,
reset ALL history locals before assigning `binding_generation`:

| # | Site (tui_entry.rs)                          | binding_generation assign | Locals currently reset (manual)                                                                 | Missing reset                                           |
|---|----------------------------------------------|---------------------------|--------------------------------------------------------------------------------------------------|---------------------------------------------------------|
| 1 | Ctrl-G session confirm (Sessions page)       | L6006                     | L5996-6002: history_window, pending, active, nav_req, nav_revision(NOT bumped), blocked, tail, browsing | history_navigation_revision NOT bumped                  |
| 2 | Sessions page row click (mouse)              | via apply_close_tab_result L5019, L5029 | same as #7 (apply_close_tab_result)                                                          | see #7                                                  |
| 3 | /new from chat (tab_hovered=Some / Ctrl-N)   | L6966                     | L6941-6951: history_window, pending, active, nav_req, blocked, tail, browsing                  | history_navigation_revision NOT bumped                  |
| 4 | /cd                                          | L7051                     | L7044-7050: history_window, pending, active, nav_req, blocked, tail, browsing                 | history_navigation_revision NOT bumped                  |
| 5 | Session create from Home (Ctrl-G, no live)   | L7213                     | L7208-7211: history_window, live, binding_generation                                            | DOES NOT reset history_pending/active/nav_req/blocked/tail/browsing at all |
| 6 | commit_child_route (child open / return parent) | L7552 (`*binding_generation = generation`) | L7551: history_window only                                                    | NO reset of pending/active/nav_req/blocked/tail/browsing/revision |
| 7 | apply_close_tab_result (tab close / select)  | L7715 (`*binding_generation = result.generation`) | L7710: history_window only                                          | NO reset of pending/active/nav_req/blocked/tail/browsing/revision |

Hook requirement: every G0 site must zero the following locals atomically
with the generation commit. Use `reset_binding_locals` from the helper (adapted
for the extra Rust locals not in the helper struct):

```
history_pending           = false
history_active            = None
history_navigation_request = None
history_navigation_revision = wrapping_add(1)
blocked_history_scroll    = None
history_tail_refresh_pending = false
history_browsing          = false
history_window            = Some(NativeHistoryWindow::new(...))  // already done at each site
```

NOTE: The helper's `reset_binding_locals` covers `pending`, `active`,
`queued`, `blocked`, and bumps `revision`. The native locals additionally need
`history_navigation_request = None` and `history_navigation_revision` bumped
and `history_browsing = false` and `history_tail_refresh_pending = false`.
Site #5, #6, #7 are currently MISSING most resets -- this is the bug class
the hook checklist addresses.

## G1. History response acceptance gate -- MUST use retire_matching-equivalent

Site: tui_entry.rs L3619-3621 (current logic)

Current:
```rust
if let Ok((request, result)) = history_rx.try_recv() {
    if history_active == Some(request) && request.generation == binding_generation {
        history_pending = false;
        history_active = None;
```

The `&&` condition is a partial guard. It checks generation equality but
does NOT verify that `request.id` still matches `history_active.id` in the
case where a binding reset happened BETWEEN request admission and response
receipt. Specifically: if a binding reset set `history_active = None` then a
new request B was admitted with a new id, a late response A (old id, old
generation) arriving in the same `try_recv` tick would be rejected by
`request.generation == binding_generation` -- BUT if A's generation happens
to equal the new generation due to reuse or if the check is refined, the id
mismatch is the critical guard.

Hook: replace the acceptance condition with `retire_matching`:

```rust
if let Ok((request, result)) = history_rx.try_recv() {
    if retire_matching(&mut history_active, &mut history_pending, request.id)
        && request.generation == binding_generation {
        // accepted -- proceed to window mutation
    }
    // else: stale/late completion, silently drop
```

This makes the invariant structural: `retire_matching` returns true ONLY when
the completion ID still names the active request, so a late A after binding
reset B can never clear B. The generation check remains as a secondary guard.

## G2. History request admission sites -- MUST carry binding_generation

Every site that sets `history_active = Some(request)` must capture
`generation: binding_generation` into the `HistoryRequestMeta`. Verify all:

| #  | Site (tui_entry.rs)                | Line    | request.generation captured |
|----|------------------------------------|---------|-----------------------------|
| A  | Older page fetch                   | L4114   | yes                         |
| B  | Newer page fetch                   | L4140   | yes                         |
| C  | Refresh (periodic)                 | L4171   | yes                         |
| D  | Tail refresh (chat_at_tail)        | L4194   | yes                         |

All admission sites correctly embed `generation: binding_generation`. No hook
needed at admission -- the bug is at acceptance (G1) and reset (G0).

## G3. History navigation revision bump sites (NOT binding changes)

These bump `history_navigation_revision` without changing binding. They must
NOT trigger `reset_binding_locals` (doing so would evict a valid in-flight
request). They are guard-relevant for `request_revision` checks:

| #  | Trigger           | Line(s)         |
|----|-------------------|-----------------|
| 1  | Wheel down (PgDn) | L5251           |
| 2  | Wheel up (PgUp)   | L5261           |
| 3  | Ctrl-F            | L5860           |
| 4  | Ctrl-B            | L5923           |

The `history_navigation_revision` is captured at admission (G2) and checked
at acceptance (L3657, L3684, L3717, L3726, L3731, L3766) to reject stale
navigation responses. This is already correct.

## G4. Compaction completion -- NOT a binding change

Site: tui_entry.rs L4083-4101

Compaction completion fetches a fresh snapshot (L4090-4097) and updates
`live`, but does NOT bump `binding_generation`. It does NOT reset history
locals. This is correct: compaction is same-session, same-binding. A binding
reset here would evict the user's active pagination. No hook needed.

## G5. /subagents, /permission, /appearance, local page routes

Sites: L7062 (/subagents), L6679 (local pages), L7087 (/permission), L7080 (/appearance)

These do NOT change `binding_generation` or `history_window`. They are
view-only or dialog overlays. No history reset needed.

## Summary of required hooks

1. **At every G0 site (7 sites):** insert full history-local reset block
   (including `history_navigation_revision = wrapping_add(1)`) before or
   atomically with the `binding_generation = prospective` assignment.

2. **At G1 (L3621):** replace `history_active == Some(request)` identity+generation
   check with `retire_matching(&mut history_active, &mut history_pending, request.id)`
   as the primary guard, keeping `request.generation == binding_generation`
   as secondary.

3. **Audit G2/G3/G4/G5** confirmations are already correct (no change needed).

## Cross-reference: upstream TypeScript equivalent

`packages/tui/src/routes/session/history.ts` `createHistoryPrepend`:
- `pending` guard (line 10-21): rejects duplicate concurrent loads.
- `pending = undefined` only when `pending !== current` (stale check, L30/L39).
- This is the TS analog of `retire_matching`: a response only commits if
  it still owns the pending slot.

`packages/tui/src/prompt/history.tsx` (prompt history): unrelated to session
history pagination; no hook needed.

`/Users/mymac/Projects/tmp/opencode/packages/tui/src/routes/session/index.tsx`:
- `prependHistory` integration point; upstream scrolls to anchor after
  prepend. Native equivalent: `apply_history_window` (L973) + anchor
  restoration (L3702, L3749). Already correct, no hook change.
