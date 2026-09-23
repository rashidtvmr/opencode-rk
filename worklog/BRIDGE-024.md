# BRIDGE-024 dialogs_system scratchpad

Claim: SystemDialog enum covers all 21 dialog-*.tsx.
Source: TS checkout a0d9b6c (NOT 95daf90; task text stale).
Observed: all components prop-bearing or prop-less selects; generic Dialog/Toast in dialog.rs/toast.rs reused, not redefined. Titles + prop shapes evidenced per file:line in module docs.
Target: crates/opentui-bridge/src/dialogs_system.rs only. lib.rs NOT touched (scope: one file; wiring left to integrator).
Tests: 6 (per-variant carry, empty-options, oversize, disabled action, confirm actions, move/link bounds). Not run (scope forbids cargo).
Unknowns: lib.rs wiring pending; provider multi-method sub-dialogs collapsed to select variant.
