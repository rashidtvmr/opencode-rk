# BRIDGE-GAP-100 scratchpad
- claim: BRIDGE-GAP-100 ses_gap100 ok
- evidence: packages/tui/src/context/route.tsx:6-23 Route=Home|Session(sessionID)|Plugin; app.tsx:54-55 Home/Session routes; crate app_host.rs:60-66 TuiInput
- target: crates/opentui-bridge/src/app_entry.rs only, no lib.rs/Cargo.toml edits, no cargo run
- tests: home_label, onboard_default_false, session_ok, session_empty_errs, session_id_truncates
- done: pending file write + rustfmt --check
