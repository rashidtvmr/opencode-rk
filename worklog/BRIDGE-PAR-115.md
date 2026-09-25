# BRIDGE-PAR-115 scratchpad (ses_par115)

Claim: in-progress via completion_claims claim().
Source evidence:
- crates/cli/src/tui_entry.rs:452 native_page_lines sig (title/model/draft/transcript/width/height)
- tui_entry.rs:477-481 title+status+rule header; :484 body_rows=height-7 tail; :506-525 Palette menu; :527-539 Context; :540-550 Help; :553-558 truncate(height)+char-safe width clip
- crates/cli/src/transcript_paint.rs + theme_utils.rs: absent (38 files glob, no match); slot/paint_lines refs not found in cli/src
Target boundary: ONE new file crates/opentui-bridge/src/page_adapter.rs. No lib.rs/Cargo.toml edits. No cargo/commit.
Tests: 6 in-file (chat tail, empty placeholder, palette strings, height trunc, multibyte width clip, labels).
Decisions: Chat frame title+status+rule+body-tail(height-7)+pad+rule+footer; fixed pages verbatim menu lines; clip via chars().take; rule width.min(120).
Verification: rustfmt --check PASS, 169 lines, under 180.
