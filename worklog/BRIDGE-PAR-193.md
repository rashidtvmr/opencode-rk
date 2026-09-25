# Claim: BRIDGE-PAR-193 ses_par193
# Target: crates/opentui-bridge/src/chat_screen.rs (new, std-only)
# Truth: paint_callsite.rs:12 build_chat_lines; session_header.rs:23 header_lines; prompt_assemble.rs:22 draft_line (read-only)
# Decision: single source build_chat_lines only; chat_lines delegates; with_toast replaces last line, char-safe clip
# Tests: delegate-eq, none-unchanged, some-replaces-last, clip-width, empty-transcript-placeholder, unicode-safe
# Status: implementing
