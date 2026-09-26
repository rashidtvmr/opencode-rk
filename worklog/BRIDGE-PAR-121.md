# BRIDGE-PAR-121 scratchpad

claim: ses_par121 via tools/completion_claims.py claim() ok.
source: /home/rashid/projects/opencode/packages/tui/src/theme/assets/ ls sorted = 33 basenames; opencode.json:1-30, tokyonight.json:1-30 read.
target: crates/opentui-bridge/src/theme_assets.rs only. No lib.rs/Cargo.toml/theme.rs edits, no cargo/commit.
tests: in-file 6 tests (count, opencode, tokyonight+dracula, unknown, fallback known/unknown).
decisions: sorted const list; fallback_chain returns [name,"opencode"] or ["opencode","opencode"]; std-only, forbid(unsafe_code), 90 lines.
evidence: rustfmt --check PASS.
