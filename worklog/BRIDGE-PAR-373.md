# BRIDGE-PAR-373: session sidebar open/pinned state

Status: UNCLAIMED (orchestrator owns tasks/completion/claims.json; proceeded file-only per lane order).

Claim: none taken. No ledger touch.

Source evidence:
- TS truth: /home/rashid/projects/opencode/packages/tui/src/routes/session/sidebar.tsx:12 (`Sidebar(props: {sessionID, overlay?})`), :26-37 (`Show when={session()}`, overlay box panel)
- Sibling pattern: crates/opentui-bridge/src/route_sidebar.rs:1 (`#![forbid(unsafe_code)]`), :33-38 (`SidePanel {open, section}`), :50-52 (`toggle`), :72+ (`#[cfg(test)]`)

Observed scenario: route-level sidebar needs open flag + pin count, distinct from `SidePanel` (open + section enum).

Target boundary: ONE new file crates/opentui-bridge/src/route_sess_side_full.rs. No lib.rs, no Cargo.toml edits.

Tests: toggle_flips, pin_bumps, pin_saturates (in-file, 3 tests).

Decisions:
- `pin()` = `saturating_add(1)` only, no open side-effect (task says "pinned bump").
- `is_open()` const fn returning `self.open`.
- `new()` const fn, Default derive; 67 lines total.

Remaining unknowns: none for lane scope. Wiring into lib.rs is orchestrator/integration job.
