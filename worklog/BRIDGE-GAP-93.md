# BRIDGE-GAP-93
claim: ses_gap93 owns BRIDGE-GAP-93.
source: TS run/stream.ts:141-175 writeSessionOutput commit flow; crate run_stream.rs:30-86 StreamBuf naming/caps.
target: crates/opentui-bridge/src/run_stream_main.rs only.
tests: in-file mod tests: push_ok, done_errs, finish_idempotent, text_concat, text_cap.
decisions: std-only, forbid(unsafe_code), Vec<String> chunks, Result<(),String> errs, chars().take cap.
status: file written, rustfmt clean, 133 lines.
