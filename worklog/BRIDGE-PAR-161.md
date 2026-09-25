# BRIDGE-PAR-161 scratchpad

claim: BRIDGE-PAR-161 ses_par161 in-progress.
source: epilogue.tsx:1-6 (createSimpleContext set(value?: string)); title_epilogue.rs (title trunc + epilogue_line, not edited).
target: crates/opentui-bridge/src/epilogue_ctx.rs only.
tests: 6 in-file (hidden empty, show renders, hide empties, 16-line cap, 512 trunc, 8KiB cap).
decisions: struct {lines, shown}; push truncates chars, false at cap; render hidden->"", join \n, byte-cap 8KiB char-boundary.
verify: rustfmt --check PASS, 124 lines (<130). No cargo per scope.
unknowns: none.
