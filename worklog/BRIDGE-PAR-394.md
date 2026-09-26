# BRIDGE-PAR-394 scratchpad (UNCLAIMED - orchestrator owns claims.json)

- Task: BRIDGE-PAR-394, one file `crates/opentui-bridge/src/toast_tsx_full.rs`.
- No ledger claim (per delegation: do NOT touch tasks/completion/claims.json).
- Source evidence:
  - `/home/rashid/projects/opencode/packages/tui/src/ui/toast.tsx:7-12` ToastOptions message + variant info|success|warning|error.
  - `crates/opentui-bridge/src/toast_ui_full.rs:9-13` ToastUi msg cap 512 + level String; view never mutates.
- Target: `ToastTsx { msg: String cap 256, level: u8 }` + `show(&mut self,&str,u8)` + `clear(&mut self)` + `line(&self)->String` cap 256. std-only, forbid(unsafe_code), <80 lines, >=3 tests.
- Tests: default_is_empty_info, show_stores_level_and_caps_msg, clear_resets.
- Decisions: numeric level passthrough (0 info,1 success,2 warning,3 error); `line` = `level: msg` char-safe take(256), mirrors toast_ui_full.rs:43-48.
- Unknowns: level->variant mapping not enforced; wiring into lib.rs left to orchestrator.
