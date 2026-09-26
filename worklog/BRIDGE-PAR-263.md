# Claim: BRIDGE-PAR-263 ses_par263
# Source evidence:
# - crates/opentui-bridge/src/sdk_stream.rs:17 SdkStream, :27 push, :43 drain_text
# - crates/opentui-bridge/src/stream_drain.rs:23 feed_sdk(sdk,bodies), :12 drain_frames
# Observed: no SdkFlow exists; send path split across feed_sdk + manual count.
# Target boundary: ONE new file crates/opentui-bridge/src/sdk_stream_full.rs only.
#   No edit to lib.rs, Cargo.toml, sdk_stream.rs, stream_drain.rs. No cargo/commit.
# Tests: in-file unit tests (>=4), rustfmt --check only.
# Decisions: pub fields sdk/sent per spec; send() always bumps by bodies.len()
#   even if sdk closed/full (count attempted, feed_sdk drops). std-only, forbid unsafe.
# Remaining: write file, rustfmt check, flip completed.
