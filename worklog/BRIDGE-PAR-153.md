# BRIDGE-PAR-153 scratchpad

claim: BRIDGE-PAR-153 via ses_par153, worklog/BRIDGE-PAR-153.md
source: packages/tui/src/context/editor.ts (labelState pending/sent/none, selectionSent flag); crates/opentui-bridge/src/editor_bridge.rs (trunc, MAX_FILE_BYTES=512, forbid unsafe, test style)
observed: editor.ts selectionSent bool drives pending/sent; bridge needs minimal pending-slot ctx, no IO
target: ONE new file crates/opentui-bridge/src/editor_ctx.rs, std-only, forbid(unsafe_code), <120 lines
tests: empty false, ack flow, double-ack false, trunc, default empty, ack-without-request false (6)
decisions: mirror editor_bridge.rs trunc; request_open empty->false; ack pending->false true else false; file() getter; private fields
unknowns: lib.rs wiring out of scope (task forbids editing)
