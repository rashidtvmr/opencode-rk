# BRIDGE-PAR-301 scratchpad
claim: BRIDGE-PAR-301 ses_par301 ok.
source: `packages/tui/src/util/provider-origin.ts:1-7` only `isConsoleManagedProvider`; no origin helpers in TS truth, new behavior.
target: `crates/opentui-bridge/src/provider_origin_full.rs` only; no lib.rs/Cargo.toml edits.
impl: origin_label (trim, empty->local, chars cap 64), is_remote (`://` with non-empty scheme), short_origin (after `://`, cut at /?#, cap 64, empty->local).
tests: 4 (empty->local, scheme gate, strip path, 64 cap).
verify: rustfmt --check only.
