# BRIDGE-PAR-129 sdk_stream
claim: ses_par129 in-progress
truth: packages/tui/src/context/sdk.tsx:82-104 SSE for-await loop, handleEvent queue+batch flush
target: crates/opentui-bridge/src/sdk_stream.rs SdkEvent/SdkStream push/close/drain_text
tests: 6 in-file (push_ok, closed_blocks, drain_concat_clear, drain_keeps_error_done, caps, drain_64k)
verify: rustfmt --check EXIT 0, 131 lines (<160), std-only forbid(unsafe_code)
