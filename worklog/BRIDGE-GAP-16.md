# BRIDGE-GAP-16
claim: ses_gap16 held, completed.
owned: crates/opentui-bridge/src/loop_events.rs (176 lines, forbid unsafe, std-only).
src: input.rs:45 InputEvent; input_events.rs parse_one/PASTE_START/PASTE_END; cli native_host.rs:140 HostEvent (read-only).
design: HostAction {Key(u32), Paste(String), Resize, Quit, Page(Palette/Context/Help/Chat), Submit}; map_event drops mouse/focus; drain_bytes fenced-paste + fail-closed.
tests: arrows_map_to_keys, paste_fenced_by_markers, quit_byte_quits, resize_passthrough, invalid_yields_none, multibyte_key.
verify: rustfmt --edition 2021 + rustfmt --check clean. No cargo/lib.rs/commit per role.
remain: integrator prewires pub mod loop_events.
