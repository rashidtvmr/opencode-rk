# BRIDGE-PAR-383 (unclaimed, file-only per orchestrator)

Claim: skipped ledger claim per task instruction (orchestrator owns claims.json).
Source: packages/tui/src/context/path-format.tsx:15-24 (formatPath), runtime.tsx:3-8 (abbreviateHome).
Observed: formatPath = cwd-relative else abbreviateHome; abbreviateHome = home-relative, `~` on exact, passthrough on `..`/absolute.
Target: crates/opentui-bridge/src/ctx_pathfmt_full.rs, std-only, forbid(unsafe_code), <70 lines.
Tests: tilde_home, no_false_prefix, base_cases, caps (>=3, in-file).
Decisions: fmt_path = abbreviateHome only (no cwd arg per deliverable sig); boundary-safe prefix; `/` home owns absolutes; char-based caps 128/64.
Unknowns: none. No cargo, no commit, lib.rs untouched.
