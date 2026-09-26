# BRIDGE-PAR-350

Unclaimed (ledger overflow; orchestrator owns claims.json). File-only lane, no claim.

Claim: in-memory text clipboard mirroring TS write(text).
Evidence: /home/rashid/projects/opencode/packages/tui/src/clipboard.ts:120-124 (write),
:76-94 copyCommand routing (out of scope, lives in clipboard.rs).
Existing: clipboard.rs (routing policy), clipboard_ctx.rs (8KiB seq variant); this file is
spec'd 64KiB plain Clipboard, no seq, copy returns ().
Target: crates/opentui-bridge/src/clipboard_full.rs, std-only, forbid(unsafe_code), <90 lines.
Tests: roundtrip, empty_roundtrip, truncates_overlong, clear_empties (4).
Decisions: len = chars().count(); truncation char-boundary (UTF-8 safe); empty copy stores empty.
Unknowns: none. lib.rs wiring is orchestrator's (not touched per scope).
