# BRIDGE-PAR-421 scratchpad (UNCLAIMED: orchestrator owns claims.json, file-only lane)

- Claim: not claimed in ledger per task orders (do NOT touch tasks/completion/claims.json). Proceeded file-only.
- Source evidence:
  - `/home/rashid/projects/opencode/packages/tui/src/feature-plugins/home/footer.tsx:1-40` Directory/Mcp/version footer line.
  - `crates/opentui-bridge/src/home_footer.rs:20-23` HomeFooter rotator; `home_footer_full.rs:9-11` HomeFootFlow wrapper.
  - Style model: `crates/opentui-bridge/src/bg_pulse_tsx_full.rs:1-36` tsx-level wrapper + forbid(unsafe_code).
- Observed scenario: TS footer lays out Directory/Mcp/spacer/Version; tips rotate in footer line.
- Target boundary: ONE new file `crates/opentui-bridge/src/home_footer_tsx_full.rs`. No lib.rs/Cargo.toml/home_footer.rs/home_footer_full.rs edits. No cargo, no commit.
- Tests: set_and_get, caps_256, clear_empties (inline, >=3).
- Decisions: HomeFoot { text: String } private + set/text_of/clear; char-based 256 cap; Default for empty; ponytail single-string note.
- Remaining unknowns: none; needs orchestrator wiring into lib.rs.
