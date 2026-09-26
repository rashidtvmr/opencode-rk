# BRIDGE-PAR-293 scratchpad
- claim: ses_par293, ledger ok
- TS truth: /home/rashid/projects/opencode/packages/tui/src/util/error.ts (errorMessage first-nonempty, errorFormat fallback)
- sibling style: crates/opentui-bridge/src/error_format.rs (forbid unsafe, must_use), format_util_full.rs
- target: crates/opentui-bridge/src/error_util_full.rs only, no lib.rs/Cargo.toml edits
- impl: short_error (first line, trim, chars cap 256), is_retryable (lower contains rate/timeout/timed out + 429/503/econn), retry_hint (retry|fix input)
- tests: 5 planned
- unknowns: none (retry substrings literal per spec)
