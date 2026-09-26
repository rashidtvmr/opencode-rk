# BRIDGE-072 scratchpad

Claim: tui_utils.rs misc TUI utils port.
Evidence:
- signal.ts:19-51 createFadeIn show/enabled alpha 0->1 reveal-once, 160ms smoothstep 16ms frames
- system.ts:15-20 describeTerminal TERM_PROGRAM[ VERSION][ in tmux/screen]
- selection.ts:46-77 handleSelectionKey ctrl+c/escape clear, focus guard keep
- renderer.ts:3 destroyRenderer clear-title + isDestroyed idempotent
- record.ts:1 isRecord object non-null non-array
- editor.ts:12-24 normalizePromptContent single trailing newline iff body newline-free
Tests: 9 in-file (fade reveal-once, disabled jump, smoothstep midpoint,
terminal format/fallbacks, selection clear/noop, focus guard, destroy once,
record guard, prompt normalize). NOT cargo-run (scope forbids cargo).
Unknowns: none.
