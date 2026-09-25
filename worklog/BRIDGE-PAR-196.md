# BRIDGE-PAR-196 scratchpad

Claim: BRIDGE-PAR-196 via ses_par196, scratchpad worklog/BRIDGE-PAR-196.md.
Source evidence: crates/opentui-bridge/src/page_adapter.rs:10 (enum Page Chat/Palette/Context/Help), :18 (page_label).
Observed: no page_router.rs existed; pattern reference home_route.rs (forbid unsafe, std-only, tests).
Target boundary: ONE new file crates/opentui-bridge/src/page_router.rs; no lib.rs/Cargo.toml edits; no cargo/commit per task.
Tests: 5 tests (new_holds_page, show_switches_page, label_delegates, is_chat_only_on_chat, default_is_chat).
Decisions: manual Default impl (Page has no Default, derive impossible); ponytail: no navigation history/stack, add when back-nav needed.
Verification: rustfmt --check PASS, 80 lines.
