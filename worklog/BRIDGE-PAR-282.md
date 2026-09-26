# BRIDGE-PAR-282 scratchpad

Claim: BRIDGE-PAR-282 via cc.claim session ses_par282. Status in-progress at start.

Source evidence:
- TS truth: /home/rashid/projects/opencode/packages/tui/src/ui/toast.tsx:7-12 (ToastOptions message + variant info|success|warning|error, single current toast).
- Sibling pattern: crates/opentui-bridge/src/toast_line.rs:1-22 (char-safe clip, no ellipsis).

Target boundary: ONE new file crates/opentui-bridge/src/toast_ui_full.rs. No lib.rs, no Cargo.toml edits. No cargo, no commit.

Tests: 5 in-file (default empty, show warn/error, unknown fallback, 512 cap, char-safe clip).
Decisions: subset variant to info|warn|error per task spec; unknown falls back info; level pre-clipped 16 chars before match; line = `level: msg` char-clipped.
Unknowns: none.

Verification: rustfmt --check PASS (FMT_OK after rustfmt applied 2 blank-line diffs), 101 lines <110, forbid(unsafe_code), std-only.
