# BRIDGE-GAP-101 scratchpad

Claim: model id helpers in `crates/opentui-bridge/src/util_model.rs`.
Source evidence:
- TS truth `packages/tui/src/util/model.ts:3-6` parse splits on `/`, provider=first, model=rest joined.
- `crates/opentui-bridge/src/model_ref.rs:30-38` ModelRef::parse (read-only, not edited).
Target boundary: ONE new file util_model.rs; no lib.rs/Cargo.toml/model_ref.rs edits; no cargo; no commit.
Tests: split_ok, split_keeps_extra_slashes, split_no_slash_empty_provider, label_format, invalid_empty_and_bare, invalid_overlong (6 in-file).
Decisions: missing `/` -> ("", id) per task spec (differs from model_ref bare->provider, documented in header); overlong bound 256 on full id; std-only forbid(unsafe_code).
Unknowns: none.
