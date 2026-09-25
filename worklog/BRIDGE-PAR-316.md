# BRIDGE-PAR-316 scratchpad

claim: ses_par316 owns BRIDGE-PAR-316, file crates/opentui-bridge/src/dialog_status_full.rs
evidence: dialog-status.tsx:10 DialogStatus (Status + rows); dialog_confirm_full.rs:16 ConfirmDialog cap pattern; dialog_message_full.rs:72 lines cap pattern
scenario: new StatusDialog {title cap 128, rows cap 32 each 256} + set_title + push->bool + lines(width) clipped cap 34
tests: title_caps, push_caps_rows_and_chars, lines_clip_width_and_cap, lines_unicode_safe
decisions: std-only, forbid(unsafe_code), char-based trunc, width.max(1), truncate lines to 34. ponytail: no wrapping, clip only; add wrap when rows need soft-wrap.
unknowns: none. ledger update blocked by ClaimError claims-must-be-bounded-mapping; file+tests exist, rustfmt clean, 99 lines.
