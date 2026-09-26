# BRIDGE-052 keybind_tables

Claim: verbatim Definitions (184) + CommandMap (163) from keybind.ts @ a0d9b6c:45-240,256-420.
Evidence: name-order diff vs TS empty; defaults/descs/cmd-values spot-checked.
Scenario: object form (keybind.ts:17-25, only input_paste uses it :162), array form (:32), parse+unknownKeys (:449-464).
Boundary: str parsing in crate::keymap, strokes in crate::key_event; not redefined.
Tests: 7 (object defaults, object explicit+errs, array merge+errs, unknown reject+override apply, lookup hit/miss, counts, all-defaults-parse).
Decisions: struct Definition (name/default/prevent_default/desc) keeps single literal row; MAX_TABLE_STROKES mirrors keymap MAX_STROKES.
Unknowns: no cargo run per scope; logically green only. lib.rs wiring left to integrator.
