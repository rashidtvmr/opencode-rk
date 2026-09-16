# WEB-007–012 accept note — GREEN, NOT ACCEPTED boundaries held

Head: `248f519`. Dirty tree noted but lane files/tests untouched (hashes match prior worklogs).
Memory at run: 2.8 GiB avail; ran one suite at a time, `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`, `timeout 120`.

## GREEN logs (this rev, exact commands)

- `cargo test -p opencode-rk-server --test transcript_lane` → `cargo test: 5 passed (1 suite, 0.00s)`
- `cargo test -p opencode-rk-sessions --test web_008_lane` → `cargo test: 5 passed (1 suite, 0.00s)`
- `cargo test -p opencode-rk-server --test turn_parts` → `cargo test: 5 passed (1 suite, 0.00s)`
- `cargo test -p opencode-rk-server --test chat_composer` → `cargo test: 5 passed (1 suite, 0.00s)`
- `cargo test -p opencode-rk-server --test web_attachments` → `cargo test: 5 passed (1 suite, 0.00s)`
- `cargo test -p opencode-rk-server --test web_tool_chooser` → `cargo test: 5 passed (1 suite, 0.00s)`

Frozen test hashes (sha256, this rev — match prior worklogs, tests NOT edited):

- `transcript_lane.rs` `f46f26de…323`
- `web_008_lane.rs` `f63116f7…031b`
- `turn_parts.rs` `b5874654…319d`
- `chat_composer.rs` `0ef115dd…02275`
- `web_attachments.rs` `328c4d65…5222d`
- `web_tool_chooser.rs` `f69d4a34…bbc60`

## Wiring boundary (holds for all six)

- `crates/server/src/lib.rs` `pub mod` list has NO `transcript_lane`, `turn_parts`, `chat_composer`, `web_attachments`, `web_tool_chooser`. Tests include them via `#[path = "../src/…"]` only.
- `crates/sessions/src/lib.rs` `pub mod` list has NO `web_008_lane`; test uses `#[path]`.
- No write-paths wired: `lib.rs` never calls `TurnParts`/`push`, `turn_contract`, `Selection::select`, `lower_composer_doc` with active chips, or attachment-send. Nothing changed to enable them this lane.

## Per-task claim / evidence / boundary / test map

### WEB-007 transcript geometry (read-only model)
- Claim: row geometry + below-message action bar + Fork popover model; frozen 5/5 GREEN.
- Evidence: `crates/server/src/transcript_lane.rs:37` `geometry_for`, `:74` `row_actions`, `:183` `popover_items` (Fork → `Branch in new chat`), `:195` `is_branch_boundary` (user/assistant only), `:202` `normalize_persisted_role` (unknown → None, row omitted), `:215` `reading_order` (identity), `:235` `PopoverState` (Esc → focus Fork), `:261` `trunc_preview` (4 KiB), `:277` `animation_ms`; bounds `MAX_PREVIEW_BYTES` 4 KiB, `MAX_ACTIONS` 8.
- Boundary: read-only. No mutation API. System/tool rows never branch boundaries; unknown roles omitted, never faked.
- Tests: T01 row/action happy path, T02 unavailable-action honesty, T03 a11y (order/focus/Esc), T04 bounds/reduced-motion, T05 legacy/streamed compat — `crates/server/tests/transcript_lane.rs`.
- Verifier must still check: browser DOM reflow 200%/400%, visible focus, keyboard popover in `web/` (vitest/tsc unexecuted).

### WEB-008 fork/branch + retry (read-only model + live server boundary owned elsewhere)
- Claim: inclusive atomic fork + single-slot retry model; frozen 5/5 GREEN.
- Evidence: `crates/sessions/src/web_008_lane.rs:140` `fork_from_message` (inclusive, atomic pre-checks), `:198` `fork_provenance`, `:217` `fork_depth`, `:242` `branch_child_title` (`Branch: ` + 512 B cap), `:255-326` `begin/fail/cancel/complete_retry` (`MAX_ACTIVE_TURNS=1`), snapshot round-trip; bounds `MAX_FORK_DEPTH=8`, `MAX_FORK_COPY_MESSAGES=500`, `MAX_TITLE=512`. Live boundary (not this lane): `crates/sessions/src/lib.rs` `branch_from_message` + `crates/server/src/lib.rs` branch route; regression `session_branch_api` previously 4/4.
- Boundary: read-only model here. System/tool rows not boundaries; failures create no partial child; parent never mutated. Project-aware scope preservation explicitly future per card.
- Tests: T01 real branch (user+assistant), T02 retry failure (no fake record), T03 a11y/navigation, T04 depth/copy/concurrency bounds + cancel, T05 reload fidelity — `crates/sessions/tests/web_008_lane.rs`.
- Verifier must still check: project/workspace scope on fork; browser navigation/focus after branch.

### WEB-009 turn parts (model exists, producer missing — stays NOT ACCEPTED)
- Claim: projection/persistence contract GREEN 5/5; end-to-end T01/T03/T05 still blocked on native producer.
- Evidence: `crates/server/src/turn_parts.rs:64` `TurnEvent` (no catch-all; unknown maps to error at call site), `:100` `ToolCall`, `:192` `push` (per-part bounds; error never touches answer), `:271` `finish`, `:308` `parts` (Reasoning/ToolActivity/Answer/References order), `:354` accessibility (collapsed + `live_politeness:"off"`), snapshot/from-snapshot (no hidden-CoT field; `chain_of_thought`/`hidden` keys rejected); bounds `MAX_EVENTS=512`, `MAX_PART_BYTES=64KiB`, `MAX_SUMMARY_BYTES=16KiB`, `MAX_REFERENCES=64`; disconnect hook fires once (`:175-183`). Live stream path `crates/server/src/lib.rs:760` `create_turn_stream` streams only `reasoning_summary_delta` + answer; `responses_history` (`:958`) REJECTS tool entries ("need a native Responses tool adapter") and blob entries ("need a native Responses attachment adapter").
- Boundary: NO `tool_calls` producer in web turn adapter; no durable citation/reference contract. Tool cards + References unavailable, not fabricated. `TurnParts` not called from `lib.rs`.
- Tests: T01 structured reconcile, T02 malformed/unknown fails closed, T03 collapsed controls + quiet streaming, T04 bounds/cancel-permit, T05 snapshot fidelity without CoT — `crates/server/tests/turn_parts.rs`.
- Verifier must still check: native tool-call producer + durable citation contract; then T01/T03/T05 end-to-end; browser suite.

### WEB-010 composer (lowering model exists, HTTP write-path for chips disabled — stays NOT ACCEPTED)
- Claim: lowering/sanitize/draft/queue model GREEN 5/5; send of unowned capabilities impossible at model level.
- Evidence: `crates/server/src/chat_composer.rs:245-258` inactive `ToolChip`/`PluginChip` → `Err(InactiveCapability)`, `Html`/`Embed` → `Err(UnsupportedNode)`; `:274` `lower_composer_doc` (count→capability→emptiness→selector→bytes), `:319` `sanitize_doc` (drops HTML/embeds/inactive chips, counts), `:349` `accessible_label`, `:397` `key_command` (Tab moves focus, Esc closes), `:430-495` draft encode/decode (unknown kinds drop+count, cap pre-parse), `:498` legacy-text compat, `:149` `DraftQueue` (cap 16, owner-steer, single live); bounds text/draft 64 KiB, nodes 256, queue 16; errors carry kinds/sizes only.
- Boundary: write-path DISABLED at HTTP layer — turn route takes plain `(model, effort, text)` only, never `ComposerDoc`; slash/mentions/queue-steer + file/tool/plugin/voice chips have no HTTP contract. Active-chip `[tool:x]` lowering exists in the pure model but is unreachable from the wire.
- Tests: T01 structured edit/send, T02 paste/format sanitize, T03 editor a11y, T04 bounds/stop/queue, T05 draft/reload compat — `crates/server/tests/chat_composer.rs`.
- Verifier must still check: HTTP contracts for slash/mentions/queue-steer; runnable `vitest`/`tsc` browser suite (unexecuted).

### WEB-011 attachments (draft ingest only, send disabled — stays NOT ACCEPTED)
- Claim: bounded draft/Library model GREEN 5/5; send path deliberately closed.
- Evidence: `crates/server/src/web_attachments.rs:142` side-effect-free `validate_attachment`, `:259` FNV-1a dedupe ingest (non-crypto key), `:292` resolve, `:304` refcounted remove, `:319-333` temp stage/abort, `:341-398` library add/reference, `:402-509` snapshot (unknown digests → length-markers, refs resolvable); bounds 8 MiB/file, 8/turn, 64 MiB temp, 256 library, 256 MiB retained; `safe_name` rejects separators/dotfiles. Server `crates/server/src/lib.rs:116-175` advertises `attachments.available_for_web_turn:false`; format-2 branch returns explicit unavailable (`:455-468`); `responses_history` rejects blob entries.
- Boundary: LEGACY draft ingest only; SEND disabled while attached (turn adapter rejects blob entries — no fake-text degradation). Screenshot/paste-drop/Library-refs + unified format-2 store + provider adapter all absent.
- Tests: T01 ingest/send-model, T02 rejection honesty, T03 picker/preview a11y model, T04 lifecycle bounds, T05 snapshot fidelity — `crates/server/tests/web_attachments.rs`.
- Verifier must still check: unified blob store, provider attachment adapter, screenshot/paste-drop/Library refs; browser suite.

### WEB-012 chooser/approvals (read-only registry, selection never serialized — stays NOT ACCEPTED)
- Claim: pure chooser/approval model GREEN 5/5; execution write-path absent.
- Evidence: `crates/server/src/web_tool_chooser.rs:118` capped deterministic `search`, `:146` `select` (disabled/denied/unknown rejected, idempotent, nothing substituted), `:182` `turn_contract` (registry order, enabled-only, no fallback injection), `:223-298` `ApprovalQueue` (no-approval/denied/disabled never mint; focus `return_to` restored), `:333-410` snapshot/rehydrate (ids+decisions+seqs only, atomic fail, pre-parse size reject); bounds caps 64 / search 32 / selection 8 / pending 16 / snapshot 4 KiB / audit 64. Server `crates/server/src/lib.rs:116-140` enumerates real `ToolRegistry` deterministic id order with `available_for_web_turn:false` + reason on every tool; plugins/approvals likewise false with reasons.
- Boundary: composer exposes registry READ-ONLY; NO selection serialized into turns; NO approval dialog faked; denied tools cannot escape via approvals; rehydrated disabled/denied ids never reactivate.
- Tests: T01 discovery/selection-contract model, T02 denied/unavailable honesty, T03 chooser/approval a11y model, T04 execution bounds, T05 snapshot/audit rehydrate — `crates/server/tests/web_tool_chooser.rs`.
- Verifier must still check: web turn tool-execution + approval-owned flow (T01/T03/T05 end-to-end); browser suite.

## What verifier must still check (all six)
1. Browser suite: `vitest`/`tsc` in `web/` unexecuted in policy worktree — DOM geometry, reflow, focus, picker/chooser/approval dialogs all unverified live.
2. WEB-009 producer + citation contract; WEB-010 HTTP chip/queue contracts; WEB-011 unified store + provider adapter; WEB-012 execution-owned approvals.
3. Integrator wiring (`lib.rs` assembly) + full regression on integrated rev.
4. No acceptance claimed here — evidence only, hashes frozen, write-paths kept disabled.
