# BRIDGE-GAP-91 scratchpad

claim: BRIDGE-GAP-91 via ses_gap91, scratchpad worklog/BRIDGE-GAP-91.md
source: TS `/home/rashid/projects/opencode/packages/opencode/src/cli/cmd/run/footer.ts` (RunFooter footer view: prompt/status/permission/question, present() swaps view + resizes footer region); naming ref `crates/opentui-bridge/src/run_footer.rs` (RunFooter, FooterView, FooterEventKind, FOOTER_CAP, append/flush/event/destroy)
target: ONE new file `crates/opentui-bridge/src/run_footer_main.rs`, no lib.rs/Cargo.toml/run_footer.rs edits, no cargo, no commit
tests: in-file 6 tests (assemble half, 256 cap, render width, short passthrough, zero width, multibyte)
decisions: char-count semantics (chars not bytes), each side min(256, width/2), render left+pad+right exact width, right-priority truncation on manual construction, std-only + forbid(unsafe_code), <160 lines
unknowns: none; rustfmt check pending
