# BRIDGE-PAR-281 scratchpad

claim: BRIDGE-PAR-281 via ses_par281, scratchpad worklog/BRIDGE-PAR-281.md
source: /home/rashid/projects/opencode/packages/tui/src/component/spinner.tsx:10 SPINNER_FRAMES 10 braille; sibling crates/opentui-bridge/src/spinner.rs (knight rider, out of scope per spinner.rs:9)
observed: spinner.tsx interval 80ms, fallback ⋯; task wants 8-frame const + Spinner cursor
target: crates/opentui-bridge/src/spinner_full.rs only; no lib.rs/Cargo.toml edits
tests: 5 tests (len8, tick wrap, frame_of sequence, new default, large frame mod)
decisions: first 8 of TS 10 frames; tick wraps mod 8; frame_of mods for safety; ponytail: no interval/fallback, caller owns
unknowns: none
