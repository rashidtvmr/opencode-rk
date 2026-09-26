# BRIDGE-PAR-242 scratchpad

Claim: BRIDGE-PAR-242 via cc.claim, session ses_par242. Ledger in-progress.

Source evidence:
- crates/opentui-bridge/src/collapse_view.rs:1 `forbid(unsafe_code)`, head+tail preview_window, marker format `... {hidden} lines collapsed ...`
- crates/opentui-bridge/src/collapse.rs:1 `forbid(unsafe_code)`, char-safe via `.chars()`, fail-closed on zero limits

Observed scenario: sibling collapse modules are small standalone units with unit tests in-file, char-safe truncation, `... N ...` markers.

Target boundary: ONE new file crates/opentui-bridge/src/collapse_full2.rs only. No lib.rs, Cargo.toml, collapse_view.rs edits. No cargo, no commit.

Tests: 7 in-file unit tests (open, collapsed count, single, empty, cap, trunc, toggle).

Decisions:
- `new(collapsed: bool)` ctor, avoids Default true/false ambiguity
- push truncates to 512 chars (char-safe, mirrors collapse.rs), rejects past 64 items with false
- collapsed visible: [] -> [], [1] -> [1] (no marker), [n] -> first + `... {n-1} more ...`
- ponytail: skipped len()/is_empty() helpers, add when callers need them
