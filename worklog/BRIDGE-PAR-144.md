# BRIDGE-PAR-144 scratchpad

claim: BRIDGE-PAR-144 via ses_par144, ledger in-progress OK.
source: TS truth dialog-timeline.tsx:10-47 DialogTimeline (user msgs, flattened text title, DialogSelect onMove); naming ref crates/opentui-bridge/src/session_timeline.rs:43-47 TimelineDialog/cursor (NOT edited, boundary kept).
target: NEW crates/opentui-bridge/src/timeline_dialog.rs only. lib.rs/Cargo.toml/session_timeline.rs untouched.
tests: 6 in-file cfg(test): open_close, cursor fwd/back wrap, selected none empty, set_entries cap count, char trunc.
decisions: set_entries resets cursor 0; trunc chars not bytes (unicode-safe); set_entries returns kept count; rem_euclid wrap matches session_timeline.rs:63-69 idiom; no cargo run per scope (rustfmt check only).
verify: rustfmt --check clean (exit 0). Line count <150.
unknowns: none.
