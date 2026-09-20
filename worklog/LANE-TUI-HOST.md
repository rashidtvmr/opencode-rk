# LANE-TUI-HOST — Native event-loop owner

## Claim
- Task: LANE-TUI-HOST (ad-hoc gap lane G4/G8, no plan story)
- Session: ses_f425545efffeG8Seym5RbOamQf
- Status: in-progress → completed on green + push

## Source evidence
- `crates/cli/src/native_app.rs:347` — `NativeApp` pure state, zero callers in main/chat/tui_entry
- `crates/cli/src/terminal_host.rs:449` — `HostLoop` pure state, same orphan status
- `crates/cli/src/native_transcript.rs`, `native_timeline.rs`, `native_composer.rs`, `native_shell.rs` — pure state
- `crates/cli/src/onboarding.rs:407` — `OnboardingSession`, same orphan status
- `crates/cli/src/tui_entry.rs:394-445` — `interactive_loop` line-only; `run()` has no native dispatch
- `crates/cli/src/main.rs:20-53` — no `mod native_host`; integrator adds it (out of lane scope)
- Gap report ses_f42575994ffeDYX9AtlvIm8b5O

## Target boundary
- Own: `crates/cli/src/native_host.rs` (new) + this scratchpad + own ledger row.
- NOT: `main.rs` mod lines, `tui_entry.rs`, frozen tests, other lanes.

## Tests (frozen at write, #[cfg(test)] in owned file)
- quit_keys_latch_once; tab_cycle_skips_collapsed_sidebar;
  live_and_disconnect_flip_badge; onboarding_gates_submit_until_done;
  routing_counters_bound_and_drop.
- Fix during authoring (pre-freeze): onboarding needs 5 advances
  (Welcome→…→Done→Main), not 4. No edits after green.

## Decisions
- `NativeHost` duplicates small enums (`Focus`, `AppView`, `SetupStep`) +
  bounds as `pub const` so file is standalone-compilable with zero
  `crate::` imports: green via `rustc --edition 2021 --test` now, and via
  `cargo check -p opencode-rk-cli --bins` once integrator adds
  `mod native_host;`. `ponytail:` dedupe against `native_app`/`onboarding`
  types once wired — single caller `tui_entry::run` then owns loop.
- Host counts routes only; retained buffers stay in owner modules (bounded).

## Remaining unknowns
- Integrator must add `mod native_host;` to main.rs and call
  `NativeHost::new/step` from `tui_entry::run`. Not this lane.
