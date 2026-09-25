# BRIDGE-PAR-408

status: unclaimed (orchestrator owns claims.json; proceeded file-only per orders)
claim: none (skipped per instruction, DO NOT touch claims.json)
task: BRIDGE-PAR-408
owned file: crates/opentui-bridge/src/util_renderer_full.rs
evidence:
- TS truth: packages/tui/src/util/renderer.ts:3 `destroyRenderer` (setTerminalTitle("") + isDestroyed guard + destroy)
- boundary: renderer_lifecycle.rs:1-7 (lifecycle mirror of same guard), safe_renderer.rs set_title (NUL guard), clip_line.rs style (forbid unsafe, tiny fn, tests)
target boundary: RendererTag name cap 64 + set + name_of + is_set; std-only, forbid(unsafe_code), <60 lines
tests: empty_unset, set_roundtrip, caps_at_64_chars
decisions: truncate by chars not bytes (unicode-safe); empty=unset mirrors title cleared on destroy; no clear() method (set("") suffices, YAGNI); ponytail ceiling: no NUL validation, add when wired to set_title
unknowns: none; lib.rs wiring left to orchestrator (out of scope)
