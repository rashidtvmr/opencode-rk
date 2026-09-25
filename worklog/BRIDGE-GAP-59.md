# BRIDGE-GAP-59 scratchpad
claim: BRIDGE-GAP-59 ses_gap59 worklog/BRIDGE-GAP-59.md
source: crates/opentui-bridge/src/renderables.rs (Focus evidence dialog-select.tsx:580-582, prompt/index.tsx:1370-1371), crates/opentui-bridge/src/toast.rs (MAX_TOASTS=8 FIFO drop-oldest, ui/toast.tsx single currentToast vs queue companion)
target: crates/opentui-bridge/src/render_fields.rs only, std-only, forbid unsafe, <170 lines
tests: focused_defaults, replace_always_true, queue_fifo, queue_cap_8, replace_vs_queue_differ (+replace_overwrites_existing)
decisions: defaults white (255,255,255) on blue (0,0,255); replace always true; queue cap 8 mirrors toast.rs MAX_TOASTS
unknowns: exact TS blue hex unevidenced, documented as default
