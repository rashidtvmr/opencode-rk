# BRIDGE-GAP-11

## Claim

- Task: `BRIDGE-GAP-11`
- Session: `ses_gap11`
- Owned file: `crates/opentui-bridge/src/solid_boundary.rs`
- Status: in progress
- Scope: std-only explicit TS-used, Rust-provided, and boundary-gap inventory. Do not edit `solid_host.rs` or `lib.rs`.

## Source evidence

- `crates/opentui-bridge/src/solid_host.rs:2-7`: boundary mirror is evidence only; no Solid runtime.
- `solid_host.rs:9-12,22`: used TS evidence names `render`, `TimeToFirstDraw`, and `extend`; `useRenderer` is named in the inventory target.
- `solid_host.rs:33-58`: Rust-owned bounds and `HostCaps::all_used()`.
- `solid_host.rs:60-77`: enabled names are `slots`, `portal`, `keyboard`, `dimensions`.
- `solid_host.rs:185-198`: `TimeToFirstDraw` is a local helper, not proof of a native Solid implementation.
- `crates/opentui-bridge/src/lib.rs:69`: `pub mod solid_host`; no direct Solid TS runtime export.
- `BRIDGE_MIGRATION_DETAIL.md:29-34,161-169`: Solid runtime, `render`, `useRenderer`, signals, and first-draw behavior remain TS-side; prior `solid_host` claims were incomplete.
- Exact repository revision inspected: `d460eb965b200e1454941db5be200b548746dea9`.

## Target contract

- `USED_BY_TS`: exact names `render`, `useRenderer`, `TimeToFirstDraw`, `extend`, `createSignal`, `createMemo`, `createEffect`, `createStore`.
- `PROVIDED_BY_RUST`: exact enabled `HostCaps::all_used()` names `slots`, `portal`, `keyboard`, `dimensions`.
- `GAP`: set difference, including the seven/eight TS names above; do not claim `TimeToFirstDraw` or `SlotId` as a Solid implementation or invent IDs.
- `all_used()` returns the Rust-provided slice; `gap()` returns the explicit gap slice.

## Tests

- In-file tests: inventory constants, exact difference, non-empty gap includes `render` and `useRenderer`, and no invented `SlotId`/other ID values.
- RED before implementation, then standalone `rustc --edition 2021 --test`; no cargo per lane instruction.

## Decisions

- Keep module self-contained and std-only; no `lib.rs` wiring in this lane.
- Do not reinterpret Rust helper structs as evidence of TS API coverage.

## Verification

- RED: `rtk rustc --edition=2021 --test crates/opentui-bridge/src/solid_boundary.rs -o /home/rashid/.cache/bun-tmp/opencode/solid_boundary_red`, then `rtk /home/rashid/.cache/bun-tmp/opencode/solid_boundary_red --test-threads=1`: 2 passed, 3 failed as expected on empty inventory.
- GREEN: `rtk rustc --edition=2021 --test crates/opentui-bridge/src/solid_boundary.rs -o /home/rashid/.cache/bun-tmp/opencode/solid_boundary_green && rtk /home/rashid/.cache/bun-tmp/opencode/solid_boundary_green --test-threads=1`: 5 passed, 0 failed.
- Format: `rtk rustfmt --check crates/opentui-bridge/src/solid_boundary.rs`: clean.
- Library compile: `rtk rustc --edition=2021 --crate-type lib crates/opentui-bridge/src/solid_boundary.rs -o /home/rashid/.cache/bun-tmp/opencode/libsolid_boundary.rlib`: passed.
- Whitespace: `rtk git diff --check -- crates/opentui-bridge/src/solid_boundary.rs worklog/BRIDGE-GAP-11.md`: clean.
- Final file SHA-256: `b91ea63e6698bd319d9a903a07a7cb8ccfdf9fab067d749437342acbfb56ca4b`.
- No cargo run, per lane instruction.

## Remaining unknowns

- Integrator must add `pub mod solid_boundary;` if the new module is needed through the crate root.
