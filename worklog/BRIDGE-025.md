# BRIDGE-025 prompt_composer

Claim: pure composer state (buffer/cursor/attachments/slash/busy/paste).
Source: index.tsx:927-957 gating, :1180-1218 pasteInputText, :1393-1417 onPaste,
:1221-1267 pasteAttachment; autocomplete.tsx:676-700 trigger; display.ts:1-10 width.
Observed: paste = normalize CRLF/CR, blank falls to clipboard path, preventDefault
+ manual insert, never submits; submit blocked by busy/disabled/autocomplete/empty.
Target: prompt_composer.rs, forbid(unsafe), std only, MAX_VALUE=65536, MAX_ATTACHMENTS=16.
Tests: 10 (emoji insert/backspace, max bound, paste accept/truncate/reject,
bracketed-paste data-only, busy gate, attachment cap, slash trigger).
Decisions: char-offset cursor (emoji-safe); slash_open hint only, caller enforces
submit block; paste truncates on char boundaries, blank rejected.
Unknowns: no cargo run per scope; logically green, needs gate + lib.rs wiring by owner.
