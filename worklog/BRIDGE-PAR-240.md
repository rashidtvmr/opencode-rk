# BRIDGE-PAR-240

claim: BRIDGE-PAR-240 via cc.claim session ses_par240 OK
source: crates/opentui-bridge/src/toast_center.rs:15 ToastCenter, :43 show, :66 current; toast_line.rs:13 toast_line, :20 toast_push_show
observed: single visible slot + bounded queue; view clips char-safe
target: ONE file crates/opentui-bridge/src/toast_full2.rs only; no lib.rs/Cargo.toml edits
tests: 5 tests in-file (new_is_empty, show_bumps_counter, line_delegates_to_center, line_none_when_empty, counts_queued_shows)
decisions: pub fields center/shown + shown() accessor; saturating_add; ponytail no dismiss passthrough
unknowns: none
