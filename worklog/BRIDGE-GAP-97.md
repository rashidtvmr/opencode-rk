# BRIDGE-GAP-97
claim: ses_gap97
TS: packages/tui/src/prompt/history.tsx (MAX_HISTORY_ENTRIES=50,move cursor,append dedup) + packages/tui/src/prompt/frecency.tsx (score freq/decay,update bumps freq)
file: crates/opentui-bridge/src/prompt_history.rs 165 lines forbid(unsafe_code) std-only
api: HistEntry{text,uses} PromptHistory{record,suggest,prev,next} caps 4KiB/200/8
fmt: rustfmt --check PASS
tests: 6 in-file, unrun per scope (no cargo)
