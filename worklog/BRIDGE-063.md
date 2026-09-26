# BRIDGE-063 dialog alert/confirm/help extras

Claim: dialog.rs additive extension only; no existing items touched.
Sources: dialog-alert.tsx:6-57, dialog-confirm.tsx:9-108 (a0d9b6c), dialog-help.tsx:6-40.
Observed: Dialog/Kind/Result existed; Alert confirm action, Confirm label/result/active, Help text(shortcut) missing.
Target: crates/opentui-bridge/src/dialog.rs only; AlertDialog, ConfirmDialog+ConfirmButton, HelpDialog+help_text, KEY_* consts; a11y labels exact ("ok","Help","cancel"/custom).
Tests: 6 new (alert keys/label, alert none, confirm actions/default, toggle/cancel-text, result shape, help text/keys); existing 4 untouched. Logical green (cargo run forbidden by scope).
Decisions: closures as action-id strings (documented divergence); esc label-only for alert/confirm noted; confirm label = cancel-button override only.
Unknowns: none; needs cargo test by integrator.
