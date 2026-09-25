# BRIDGE-PAR-310 scratchpad

claim: BRIDGE-PAR-310 via cc.claim, session ses_par310, scratchpad worklog/BRIDGE-PAR-310.md. ok.
source evidence:
- TS truth: packages/tui/src/component/todo-item.tsx:19 glyph `[completed?"x":...]` + content text.
- style ref: crates/opentui-bridge/src/toast_line.rs:1 forbid(unsafe_code), char-safe clip, fn + tests.
observed: tsx has 3 statuses; task wants bool done only.
target boundary: ONE new file crates/opentui-bridge/src/todo_item_full.rs. no lib.rs, no Cargo.toml, no cargo, no commit.
tests: 5 tests in-file (unchecked_line, toggle_marks_done, toggle_twice_unchecks, title_capped_at_256, empty_title_line).
decisions: title truncate at new() via chars().take(256); line() format `[x]/[ ] + title`; ponytail note for status enum.
remaining: rustfmt --check, then mark completed.
