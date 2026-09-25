# BRIDGE-PAR-180 scratchpad

claim: BRIDGE-PAR-180 via ses_par180, worklog/BRIDGE-PAR-180.md
source: crates/opentui-bridge/src/toast_center.rs:15 ToastCenter, :66 current()->Option<&str>, :43 show(&mut self,&str)
observed: single current slot + bounded queue; no line view exists
target: ONE new file crates/opentui-bridge/src/toast_line.rs, toast_line+toast_push_show, std-only, forbid(unsafe_code), <100 lines, >=4 tests
tests: 5 unit tests in-file (none-when-empty, full-when-fits, clips-to-width, clip-char-safe, push-show-delegates)
decisions: chars().take(width) char-safe clip; push delegates to show; no ellipsis/padding per ponytail note
unknowns: none; lib.rs wiring out of scope (owner pre-wires)
