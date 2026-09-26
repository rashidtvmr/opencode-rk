# BRIDGE-GAP-107

Claim: cc.claim BRIDGE-GAP-107 ses_gap107 worklog/BRIDGE-GAP-107.md OK.
Source: crates/opentui-bridge/src/run_footer.rs:1-199 (RunFooter, FooterView, FooterEventKind, 6 tests); TS truth packages/opencode/src/cli/cmd/run/footer.ts:1-120 + types.ts:173-176 FooterView prompt/permission/question.
Boundary: EXTEND run_footer.rs append-only; lib.rs/Cargo.toml untouched; no cargo; no commit/push.
Tests: tests2 5 tests (default_is_status_idle, show_switches_view, busy_flag_toggles, prompt_detect_only_on_prompt, labels_non_empty_and_stable).
Decisions: FooterView2 Status(default)/Prompt/Permission/Question/Subagent/Menu; FooterMachine view+busy; show/set_busy/is_prompt/view_label; std-only.
Unknowns: none. Verify: rustfmt --check PASS exit 0.
