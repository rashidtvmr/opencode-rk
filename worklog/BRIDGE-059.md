# BRIDGE-059 tool_output.rs

Claim: budgets + ToolKind + InlineRow + MetaKeys + input primitive.
Evidence: index.tsx @a0d9b6c: shouldHide 1707-11; Generic 1791-98 (maxLines 3);
 InlineTool/Row 1829-85; BlockTool 1987-2037; Shell 2039-96 (maxLines 10 at 2046);
 Glob 2135-46; Read 2148-81; Grep 2183-94; WebFetch 2196-2202; WebSearch 2204-11;
 Task 2213-2309; Execute 2342-86 (preview 4 at 2349); Edit 2388-39;
 ApplyPatch 2441-2515; TodoWrite 2517-41; Question 2543-77; Skill 2579-85;
 input() 2613-20; toolDisplays/toolDisplay 2630-49; parses/diags 2656-2710.
 Scenarios: hide completed when details off; budgets 3/10/4; 14-kind dispatch,
 unknown -> Generic fallback; pending `~ msg`, denied strikethrough, failed
 keeps text (color-only upstream); evidenced meta keys; primitive bounded.
 Boundary: color/spinner/click/width-char budgets renderer-owned; char budget
 formula lives in collapse.rs; tool_display.rs ToolDisplay reused for bound.
 Tests: 9 tests in-file. Decisions: icon const per kind, Task/Execute
 non-final default; `matches` not in META_KEYS (Grep-only). Unknowns: none.
