# BRIDGE-PAR-287 scratchpad

claim: BRIDGE-PAR-287 via completion_claims session ses_par287.
source evidence:
- TS truth packages/tui/src/util/format.ts:1-20 (formatDuration only; not this lane's target).
- TS formatBytes: packages/opencode/src/session/tools.ts:584 (`B|KB|MB`, ceil) adapted to spec B|KiB|MiB|GiB 1 decimal.
- TS number: packages/tui/src/util/locale.ts:30-37 (`M/K` suffixes) adapted to plain/k 1 decimal.
- TS pluralize: locale.ts:81-84 (`{}` template) adapted to `N one|many`.
- sibling style: crates/opentui-bridge/src/format.rs:1-11 (forbid unsafe, doc header, must_use, cfg test mod).
target boundary: ONE new file crates/opentui-bridge/src/format_util_full.rs only. No lib.rs/Cargo.toml edits.
tests: 4 test fns (bytes_units, count_plain_and_k, plural_picks_form, bytes_large_gib) inline cfg(test).
decisions: binary 1024 divisors, f64 one-decimal; GiB open-ended; format_count k only (no M per spec).
verification: rustfmt --check PASS, 75 lines (<110). No cargo per scope.
