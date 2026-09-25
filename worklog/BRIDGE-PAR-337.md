# BRIDGE-PAR-337 scratchpad (UNCLAIMED: ledger overflow, orchestrator owns claims.json, file-only per order)

## Claim
- Task BRIDGE-PAR-337, status: unclaimed (no ledger touch per instruction). Proceeded file-only.

## Source evidence
- `/home/rashid/projects/opencode/packages/tui/src/component/bg-pulse.tsx:1-50` (GoUpsellArtRenderable, painter setters, width/height/live handling)
- `crates/opentui-bridge/src/bg_pulse.rs:1-28` (PERIOD=4600, RINGS=3, envelope/breath/MAX_PULSE=0.7, FPS_PIN=30, ponytail note)
- Did NOT edit `lib.rs`, `Cargo.toml`, `bg_pulse.rs`, `bg_pulse_full.rs`.

## Observed scenario
- Needed tsx-level thin wrapper: clamped alpha scalar + fixed-width ascii bar.

## Target boundary
- ONE new file: `crates/opentui-bridge/src/bg_pulse_tsx_full.rs`, std-only, forbid(unsafe_code), <80 lines.

## Tests
- `clamps_alpha`, `bar_scales_with_alpha`, `bar_caps_at_64` (inline `#[cfg(test)]`).
- Verification: `rustfmt --check` only (no cargo per scope).

## Decisions
- `bar`: `"#".repeat(round(alpha * min(width,64)))`; NaN -> 0.0 fail-closed.
- `ponytail:` scalar + bar only; per-cell colors deferred.

## Remaining unknowns
- Wiring into `lib.rs` left to orchestrator (out of scope).
