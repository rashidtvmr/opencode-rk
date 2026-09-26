# Claim
- Task: BRIDGE-PAR-125, session ses_par125
- Owned file: crates/opentui-bridge/src/session_footer.rs (new, only file)
- Never touch: lib.rs, Cargo.toml, run_footer.rs, run_footer_view.rs

# Source evidence
- /home/rashid/projects/opencode/packages/tui/src/routes/session/footer.tsx:1-91 read fully (Switch Status/Permission/LSP/MCP, /status hint)
- crates/opentui-bridge/src/run_footer_view.rs:1-148 read (FooterSurface union incl Prompt/Permission/Question/Subagent; FooterSnapshot busy+queue; do NOT edit)

# Observed scenario
- Need session-level footer slot distinct from run_footer_view union.

# Target boundary
- SessionFooter { view: FooterSlot, busy: bool }, FooterSlot {Status,Prompt,Permission,Question,Subagent}
- show(FooterSlot), set_busy(bool), render()->String "footer <slot> idle|busy" cap 64, is_interactive()->bool
- std-only, forbid(unsafe_code), <150 lines, in-file #[cfg(test)] >=5 tests

# Tests
- default status, show, busy render, interactive set, slot roundtrip (+cap test)

# Decisions
- Lowercase slot names in render; truncate to 64 chars (format string already short; defensive truncate).
- Interactive = Prompt|Permission|Question.

# Unknowns
- None.
